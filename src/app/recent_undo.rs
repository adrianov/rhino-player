struct RecentUndoWiring {
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    do_commit: Rc<dyn Fn() + 'static>,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
}

struct RecentUndoCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::ScrolledWindow,
    flow: gtk::Box,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_close: gtk::Button,
    on_open: RcPathFn,
    want_recent: bool,
}

fn wire_recent_undo(ctx: RecentUndoCtx) -> RecentUndoWiring {
    let RecentUndoCtx {
        player,
        recent: recent_scrl,
        flow: flow_recent,
        undo_shell,
        undo_label,
        undo_btn,
        undo_close,
        on_open,
        want_recent,
    } = ctx;

    let recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>> = Rc::new(RefCell::new(None));
    let pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>> =
        Rc::new(RefCell::new(None));
    let recent_backfill_start: Rc<dyn Fn(Rc<RecentContext>, Vec<PathBuf>)> = {
        let p = player.clone();
        let pending = pending_recent_backfill.clone();
        Rc::new(move |ctx, paths| schedule_or_defer_recent_backfill(&p, &pending, ctx, paths))
    };
    {
        let rb = recent_backfill.clone();
        let pending = pending_recent_backfill.clone();
        recent_scrl.connect_destroy(move |_| {
            pending.borrow_mut().take();
            if let Some(ctx) = rb.borrow_mut().take() {
                ctx.shutdown();
            }
        });
    }

    let undo_remove_stack = Rc::new(RefCell::new(Vec::<ContinueBarUndo>::new()));
    let undo_timer = Rc::new(RefCell::new(None::<glib::source::SourceId>));
    type DismissTopRef = Rc<RefCell<Option<Weak<dyn Fn() + 'static>>>>;
    let do_commit_weak: DismissTopRef = Rc::new(RefCell::new(None));
    let ush_d = undo_shell.clone();
    let ul_d = undo_label.clone();
    let ub_d = undo_btn.clone();
    let urs_d = undo_remove_stack.clone();
    let uts_d = undo_timer.clone();
    let wk_d = do_commit_weak.clone();
    let do_commit: Rc<dyn Fn() + 'static> = Rc::new(move || {
        cancel_undo_timer(uts_d.as_ref());
        if urs_d.borrow_mut().pop().is_none() {
            return;
        }
        sync_undo_bar(&ul_d, &ub_d, &ush_d, &urs_d);
        if !urs_d.borrow().is_empty() {
            if let Some(f) = wk_d.borrow().as_ref().and_then(|w| w.upgrade()) {
                *uts_d.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
                    f();
                    glib::ControlFlow::Break
                }));
            }
        }
    });
    *do_commit_weak.borrow_mut() = Some(Rc::downgrade(&do_commit));
    let on_remove_cell: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    let on_trash_slot: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    let fr_sl = flow_recent.clone();
    let recent_rm = recent_scrl.clone();
    let op_s = on_open.clone();
    let rbf_rm = recent_backfill.clone();
    let ur_stack = undo_remove_stack.clone();
    let u_sh_rm = undo_shell.clone();
    let undo_t_rm = undo_btn.clone();
    let u_la_rm = undo_label.clone();
    let ut_rm = undo_timer.clone();
    let do_rm = do_commit.clone();
    let cell_rm = on_remove_cell.clone();
    let cell_t = on_trash_slot.clone();
    let on_trash: RcPathFn = Rc::new({
        let fr_t = fr_sl.clone();
        let rec_t = recent_rm.clone();
        let op_t = op_s.clone();
        let rbf_t = rbf_rm.clone();
        let ur_t = ur_stack.clone();
        let u_la_t = u_la_rm.clone();
        let undo_t_t = undo_t_rm.clone();
        let u_sh_t = u_sh_rm.clone();
        let do_t = do_rm.clone();
        let ut_t = ut_rm.clone();
        let cell_rm = cell_rm.clone();
        let cell_t = cell_t.clone();
        move |path: &Path| {
            if !path.is_file() {
                return;
            }
            let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            let snap = capture_list_remove_undo(path);
            if let Err(e) = gio::File::for_path(path).trash(gio::Cancellable::NONE) {
                eprintln!("[rhino] move to trash (continue card): {e}");
                return;
            }
            let in_trash = trash_xdg::find_trash_files_stored_path(&want);
            if in_trash.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            remove_continue_entry(path);
            if let Some(t) = in_trash {
                ur_t.borrow_mut()
                    .push(ContinueBarUndo::Trash { snap, in_trash: t });
                sync_undo_bar(&u_la_t, &undo_t_t, &u_sh_t, &ur_t);
                rearm_undo_dismiss(&do_t, ut_t.as_ref());
            }
            let f = cell_rm
                .borrow()
                .as_ref()
                .expect("on_remove not wired")
                .clone();
            let t = cell_t
                .borrow()
                .as_ref()
                .expect("on_trash not wired")
                .clone();
            reflow_continue_cards(&fr_t, &rec_t, op_t.clone(), f, t, &rbf_t);
        }
    });
    *on_trash_slot.borrow_mut() = Some(on_trash.clone());
    let on_remove: RcPathFn = Rc::new({
        let cell_rm = on_remove_cell.clone();
        let tslot = on_trash_slot.clone();
        let fr_sl = fr_sl;
        let recent_rm = recent_rm;
        let op_s = op_s;
        let rbf_rm = rbf_rm;
        let ur_stack = ur_stack.clone();
        let u_la_rm = u_la_rm.clone();
        let undo_t_rm = undo_t_rm.clone();
        let u_sh_rm = u_sh_rm.clone();
        let do_rm = do_rm.clone();
        let ut_rm = ut_rm.clone();
        move |path: &Path| {
            let u = capture_list_remove_undo(path);
            remove_continue_entry(path);
            ur_stack.borrow_mut().push(ContinueBarUndo::ListRemove(u));
            sync_undo_bar(&u_la_rm, &undo_t_rm, &u_sh_rm, &ur_stack);
            let f = cell_rm
                .borrow()
                .as_ref()
                .expect("on_remove not wired")
                .clone();
            let t = tslot.borrow().as_ref().expect("on_trash not wired").clone();
            reflow_continue_cards(&fr_sl, &recent_rm, op_s.clone(), f, t, &rbf_rm);
            rearm_undo_dismiss(&do_rm, ut_rm.as_ref());
        }
    });
    *on_remove_cell.borrow_mut() = Some(on_remove.clone());

    {
        let fr_u = flow_recent.clone();
        let rec_u = recent_scrl.clone();
        let op_u = on_open.clone();
        let rbf_u = recent_backfill.clone();
        let ur_u = undo_remove_stack.clone();
        let u_sh_u = undo_shell.clone();
        let undo_t_u = undo_btn.clone();
        let u_la_u = undo_label.clone();
        let ut_u = undo_timer.clone();
        let do_u = do_commit.clone();
        let cell_u = on_remove_cell.clone();
        let tslot_u = on_trash_slot.clone();
        undo_btn.connect_clicked(glib::clone!(
            #[strong]
            fr_u,
            #[strong]
            rec_u,
            #[strong]
            op_u,
            #[strong]
            rbf_u,
            #[strong]
            ur_u,
            #[strong]
            u_sh_u,
            #[strong]
            undo_t_u,
            #[strong]
            u_la_u,
            #[strong]
            ut_u,
            #[strong]
            do_u,
            #[strong]
            cell_u,
            #[strong]
            tslot_u,
            move |_| {
                cancel_undo_timer(ut_u.as_ref());
                let Some(undo) = ur_u.borrow_mut().pop() else {
                    return;
                };
                if let Err(e) = apply_bar_undo(&undo) {
                    eprintln!("[rhino] undo: {e}");
                    ur_u.borrow_mut().push(undo);
                    return;
                }
                history::record(undo.target_path());
                sync_undo_bar(&u_la_u, &undo_t_u, &u_sh_u, &ur_u);
                rec_u.set_visible(true);
                let f = cell_u
                    .borrow()
                    .as_ref()
                    .expect("on_remove not wired")
                    .clone();
                let t = tslot_u
                    .borrow()
                    .as_ref()
                    .expect("on_trash not wired")
                    .clone();
                reflow_continue_cards(&fr_u, &rec_u, op_u.clone(), f, t, &rbf_u);
                if !ur_u.borrow().is_empty() {
                    rearm_undo_dismiss(&do_u, ut_u.as_ref());
                }
            }
        ));
    }
    {
        let dc = do_commit.clone();
        undo_close.connect_clicked(move |_| {
            dc();
        });
    }

    if want_recent {
        let paths5: Vec<PathBuf> = history::load().into_iter().take(5).collect();
        recent_view::fill_idle(
            &flow_recent,
            paths5,
            on_open.clone(),
            on_remove.clone(),
            on_trash.clone(),
            recent_backfill.clone(),
            recent_backfill_start.clone(),
        );
    }

    RecentUndoWiring {
        recent_backfill,
        pending_recent_backfill,
        undo_remove_stack,
        undo_timer,
        do_commit,
        on_remove,
        on_trash,
    }
}

fn wire_recent_spacer_fullscreen(
    sp_empty: [gtk::Box; 4],
    win: &adw::ApplicationWindow,
    fs_restore: &Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &Rc<Cell<bool>>,
    recent: &gtk::ScrolledWindow,
) {
    for sp in sp_empty {
        let d2 = gtk::GestureClick::new();
        d2.set_button(gtk::gdk::BUTTON_PRIMARY);
        let w2 = win.clone();
        let fr2 = fs_restore.clone();
        let lu2 = last_unmax.clone();
        let sk2 = skip_max_to_fs.clone();
        let rec2 = recent.clone();
        d2.connect_pressed(move |gest, n_press, _, _| {
            if n_press != 2 || !rec2.is_visible() {
                return;
            }
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            toggle_fullscreen(&w2, &fr2, &lu2, &sk2);
        });
        sp.add_controller(d2);
    }
}

#[derive(Clone)]
struct PlayToggleCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    sub_menu: Option<gtk::MenuButton>,
}

fn toggle_play_pause(ctx: &PlayToggleCtx) -> bool {
    let g = ctx.player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
        return false;
    }
    if ctx.recent.is_visible() {
        if let Some(path) = local_file_from_mpv(&b.mpv) {
            *ctx.last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
            ctx.win.set_title(Some(title_for_open_path(&path).as_str()));
        }
        sync_window_aspect_from_mpv(&b.mpv, ctx.win_aspect.as_ref());
        resync_warm_continue(&b.mpv);
        ctx.gl.queue_render();
        drop(g);
        schedule_warm_reveal(ctx.clone());
        return true;
    }
    let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    if paused {
        let off = {
            let mut pref = ctx.video_pref.borrow_mut();
            video_pref::resync_smooth_if_speed_mismatch(&b.mpv, &mut pref)
        };
        if off {
            sync_smooth_60_to_off(&ctx.app);
        }
    }
    if b.mpv.set_property("pause", !paused).is_ok() {
        ctx.gl.queue_render();
        return true;
    }
    false
}

fn schedule_warm_reveal(ctx: PlayToggleCtx) {
    let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
        ctx.recent.set_visible(false);
        (ctx.on_video_chrome)();
        schedule_window_fit_h_video(ctx.player.clone(), ctx.win.clone());
        if let Some(button) = ctx.sub_menu.as_ref() {
            schedule_sub_button_scan(ctx.player.clone(), button.clone());
        }
        ctx.win.present();
        if let Some(b) = ctx.player.borrow().as_ref() {
            let _ = b.mpv.set_property("pause", false);
        }
        ctx.gl.queue_render();
        glib::ControlFlow::Break
    });
}

fn wire_play_toggles(play_pause: &gtk::Button, ctx: PlayToggleCtx) {
    {
        let btn_ctx = ctx.clone();
        play_pause.connect_clicked(move |_| {
            toggle_play_pause(&btn_ctx);
        });
    }

    let rpp = gtk::GestureClick::new();
    rpp.set_button(gtk::gdk::BUTTON_SECONDARY);
    rpp.set_propagation_phase(gtk::PropagationPhase::Capture);
    let gl = ctx.gl.clone();
    {
        let press_ctx = ctx;
        rpp.connect_pressed(move |gest, n_press, _, _| {
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            if n_press == 1 {
                toggle_play_pause(&press_ctx);
            }
        });
    }
    gl.add_controller(rpp);
}
