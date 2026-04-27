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

