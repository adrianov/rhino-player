fn read_value_type(
    dk_read: gtk::gdk::Drop,
    dk_finish: gtk::gdk::Drop,
    typ: glib::types::Type,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: gtk::MenuButton,
    on_open: RcPathFn,
    on_empty: impl FnOnce() + 'static,
) {
    let dk_cb = dk_finish;
    let dk_r = dk_read;
    dk_r.read_value_async(
        typ,
        glib::Priority::default(),
        None::<&gio::Cancellable>,
        move |got| match got {
            Ok(val) => {
                let paths = paths_from_gvalue(val.type_(), &val);
                if !paths.is_empty() {
                    dispatch_paths_and_finish_drop(paths, &player, &sub_menu, &on_open, &dk_cb);
                } else {
                    on_empty();
                }
            }
            Err(_) => {
                on_empty();
            }
        },
    );
}

/// Shared drop-read state: MIME list, negotiated type, and widgets/refs duplicated by every fallback step.
#[derive(Clone)]
struct DropReadCtx {
    dk: gtk::gdk::Drop,
    fin: gtk::gdk::Drop,
    negotiated: glib::types::Type,
    mimes: Vec<String>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: gtk::MenuButton,
    on_open: RcPathFn,
}

impl DropReadCtx {
    fn read_value(&self, typ: glib::types::Type, on_empty: Rc<dyn Fn()>) {
        read_value_type(
            self.dk.clone(),
            self.fin.clone(),
            typ,
            self.player.clone(),
            self.sub_menu.clone(),
            self.on_open.clone(),
            move || on_empty(),
        );
    }

    fn finish_empty(&self) {
        self.fin.finish(gtk::gdk::DragAction::empty());
    }

    /// When MIME stream is unavailable: optional `read_value` on the negotiated type, else finish.
    fn finish_negotiated_or_empty(&self) {
        if !self.negotiated.is_valid() {
            self.finish_empty();
            return;
        }
        let fin = self.fin.clone();
        read_value_type(
            fin.clone(),
            fin.clone(),
            self.negotiated,
            self.player.clone(),
            self.sub_menu.clone(),
            self.on_open.clone(),
            move || fin.finish(gtk::gdk::DragAction::empty()),
        );
    }
}

fn mime_read_phase(ctx: Rc<DropReadCtx>) {
    if !ctx.mimes.is_empty() {
        let refs: Vec<&str> = ctx.mimes.iter().map(|s| s.as_str()).collect();
        let dk2 = ctx.dk.clone();
        let fin_err = ctx.fin.clone();
        let negotiated = ctx.negotiated;
        let player = ctx.player.clone();
        let sub = ctx.sub_menu.clone();
        let open = ctx.on_open.clone();
        ctx.dk.read_async(
            &refs,
            glib::Priority::default(),
            None::<&gio::Cancellable>,
            move |mime_res| match mime_res {
                Ok((stream, mime_gs)) => {
                    let mime = mime_gs.as_str().to_owned();
                    let dk3 = dk2.clone();
                    let player_stream = player.clone();
                    let sub_stream = sub.clone();
                    let on_stream = open.clone();
                    drain_input_stream_aggregate(
                        stream,
                        Vec::new(),
                        Box::new(move |acc| match acc {
                            Ok(bytes) => {
                                let paths = paths_from_received_bytes(&bytes, &mime);
                                if paths.is_empty() && negotiated.is_valid() {
                                    let dk_nv = dk3.clone();
                                    read_value_type(
                                        dk_nv.clone(),
                                        dk_nv.clone(),
                                        negotiated,
                                        player_stream.clone(),
                                        sub_stream.clone(),
                                        on_stream.clone(),
                                        move || dk_nv.finish(gtk::gdk::DragAction::empty()),
                                    );
                                    return;
                                }
                                dispatch_paths_and_finish_drop(
                                    paths,
                                    &player_stream,
                                    &sub_stream,
                                    &on_stream,
                                    &dk3,
                                );
                            }
                            Err(_) => {
                                if negotiated.is_valid() {
                                    let dk_dup = dk3.clone();
                                    read_value_type(
                                        dk_dup.clone(),
                                        dk_dup.clone(),
                                        negotiated,
                                        player_stream,
                                        sub_stream,
                                        on_stream,
                                        move || dk_dup.finish(gtk::gdk::DragAction::empty()),
                                    );
                                } else {
                                    dk3.finish(gtk::gdk::DragAction::empty());
                                }
                            }
                        }),
                    );
                }
                Err(_) => {
                    if negotiated.is_valid() {
                        let f = fin_err.clone();
                        read_value_type(
                            f.clone(),
                            f.clone(),
                            negotiated,
                            player,
                            sub,
                            open,
                            move || f.finish(gtk::gdk::DragAction::empty()),
                        );
                    } else {
                        fin_err.finish(gtk::gdk::DragAction::empty());
                    }
                }
            },
        );
        return;
    }

    ctx.finish_negotiated_or_empty();
}

fn drop_continue_after_gfile(ctx: Rc<DropReadCtx>) {
    mime_read_phase(ctx);
}

fn drop_continue_after_file_list(ctx: Rc<DropReadCtx>) {
    if ctx.dk.formats().contains_type(gio::File::static_type()) {
        let next = {
            let c = Rc::clone(&ctx);
            Rc::new(move || drop_continue_after_gfile(Rc::clone(&c)))
        };
        ctx.read_value(gio::File::static_type(), next);
        return;
    }
    drop_continue_after_gfile(ctx);
}

fn try_read_drop_async(
    dk: gtk::gdk::Drop,
    fm_types: gtk::gdk::ContentFormats,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_menu: gtk::MenuButton,
    on_open: RcPathFn,
) {
    let negotiated = dk.formats().match_type(&fm_types);
    let mimes_owned = mime_types_ordered_for_drop_read(&dk);
    let ctx = Rc::new(DropReadCtx {
        dk: dk.clone(),
        fin: dk,
        negotiated,
        mimes: mimes_owned,
        player,
        sub_menu,
        on_open,
    });

    if ctx.dk.formats().contains_type(gtk::gdk::FileList::static_type()) {
        let next = {
            let c = Rc::clone(&ctx);
            Rc::new(move || drop_continue_after_file_list(Rc::clone(&c)))
        };
        ctx.read_value(gtk::gdk::FileList::static_type(), next);
        return;
    }
    drop_continue_after_file_list(ctx);
}
