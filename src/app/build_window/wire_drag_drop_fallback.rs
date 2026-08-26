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
    if ctx.mimes.is_empty() {
        ctx.finish_negotiated_or_empty();
        return;
    }
    let refs: Vec<&str> = ctx.mimes.iter().map(|s| s.as_str()).collect();
    let inner = Rc::clone(&ctx);
    ctx.dk.read_async(
        &refs,
        glib::Priority::default(),
        None::<&gio::Cancellable>,
        move |mime_res| match mime_res {
            Ok((stream, mime_gs)) => {
                drain_drop_stream(Rc::clone(&inner), stream, mime_gs.as_str().to_owned())
            }
            Err(_) => inner.finish_negotiated_or_empty(),
        },
    );
}

fn drain_drop_stream(ctx: Rc<DropReadCtx>, stream: gio::InputStream, mime: String) {
    drain_input_stream_aggregate(
        stream,
        Vec::new(),
        Box::new(move |acc| match acc {
            Ok(bytes) => dispatch_drop_bytes(ctx, &bytes, &mime),
            Err(_) => ctx.finish_negotiated_or_empty(),
        }),
    );
}

/// Dispatches decoded paths; an empty payload falls back to the negotiated type when one exists,
/// otherwise reaches `consume_dropped_paths` so its empty-list diagnostic still fires.
fn dispatch_drop_bytes(ctx: Rc<DropReadCtx>, bytes: &[u8], mime: &str) {
    let paths = paths_from_received_bytes(bytes, mime);
    if !paths.is_empty() || !ctx.negotiated.is_valid() {
        dispatch_paths_and_finish_drop(paths, &ctx.player, &ctx.sub_menu, &ctx.on_open, &ctx.fin);
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

    if ctx
        .dk
        .formats()
        .contains_type(gtk::gdk::FileList::static_type())
    {
        let next = {
            let c = Rc::clone(&ctx);
            Rc::new(move || drop_continue_after_file_list(Rc::clone(&c)))
        };
        ctx.read_value(gtk::gdk::FileList::static_type(), next);
        return;
    }
    drop_continue_after_file_list(ctx);
}
