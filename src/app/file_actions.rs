struct VideoFileActions {
    close_video: gio::SimpleAction,
    move_to_trash: gio::SimpleAction,
}

struct VideoFileActionCtx {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    recent: gtk::ScrolledWindow,
    flow_recent: gtk::Box,
    gl: gtk::GLArea,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    browse_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    do_commit: Rc<dyn Fn() + 'static>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
}

fn wire_video_file_actions(ctx: VideoFileActionCtx) -> VideoFileActions {
    let VideoFileActionCtx {
        app,
        player,
        win,
        recent: recent_scrl,
        flow_recent,
        gl: gl_area,
        on_open,
        on_remove,
        on_trash,
        recent_backfill,
        last_path,
        sibling_seof,
        sibling_nav,
        browse_chrome,
        win_aspect,
        undo_shell,
        undo_label,
        undo_btn,
        undo_timer,
        undo_remove_stack,
        do_commit,
        close_action_cell,
        trash_action_cell,
    } = ctx;

    let close_video = gio::SimpleAction::new("close-video", None);
    let p_btv = player.clone();
    let w_btv = win.clone();
    let recent_btv = recent_scrl.clone();
    let flow_btv = flow_recent.clone();
    let gl_btv = gl_area.clone();
    let op_btv = on_open.clone();
    let rem_btv = on_remove.clone();
    let trash_btv = on_trash.clone();
    let rbf_btv = recent_backfill.clone();
    let last_btv = last_path.clone();
    let seof_btv = sibling_seof.clone();
    let nav_btv = sibling_nav.clone();
    let browse_btv = browse_chrome.clone();
    let wa_btv = win_aspect.clone();
    let ush_btv = undo_shell.clone();
    let ula_btv = undo_label.clone();
    let uti_btv = undo_timer.clone();
    let ur_btv = undo_remove_stack.clone();
    let undo_t_btv = undo_btn.clone();
    close_video.connect_activate(glib::clone!(
        #[strong]
        p_btv,
        #[strong]
        w_btv,
        #[strong]
        recent_btv,
        #[strong]
        flow_btv,
        #[strong]
        gl_btv,
        #[strong]
        op_btv,
        #[strong]
        rem_btv,
        #[strong]
        trash_btv,
        #[strong]
        rbf_btv,
        #[strong]
        last_btv,
        #[strong]
        seof_btv,
        #[strong]
        browse_btv,
        #[strong]
        wa_btv,
        #[strong]
        ush_btv,
        #[strong]
        ula_btv,
        #[strong]
        uti_btv,
        #[strong]
        ur_btv,
        #[strong]
        undo_t_btv,
        move |_, _| {
            if recent_btv.is_visible() || p_btv.borrow().is_none() {
                return;
            }
            back_to_browse(
                &BackToBrowseCtx {
                    player: p_btv.clone(),
                    on_open: op_btv.clone(),
                    on_remove: rem_btv.clone(),
                    on_trash: trash_btv.clone(),
                    recent_backfill: rbf_btv.clone(),
                    last_path: last_btv.clone(),
                    sibling_seof: seof_btv.clone(),
                    sibling_nav: nav_btv.clone(),
                    win_aspect: wa_btv.clone(),
                    on_browse: browse_btv.clone(),
                    undo_shell: ush_btv.clone(),
                    undo_label: ula_btv.clone(),
                    undo_btn: undo_t_btv.clone(),
                    undo_timer: uti_btv.clone(),
                    undo_remove_stack: ur_btv.clone(),
                },
                &w_btv,
                &gl_btv,
                &recent_btv,
                &flow_btv,
                true,
            );
        }
    ));
    app.add_action(&close_video);
    *close_action_cell.borrow_mut() = Some(close_video.clone());
    let cv_s1 = close_video.clone();
    let p_s1 = player.clone();
    let r_s1 = recent_scrl.clone();
    recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
        sync_close_video_action(&cv_s1, &p_s1, &r_s1);
    });
    let cv_s2 = close_video.clone();
    let p_s2 = player.clone();
    let r_s2 = recent_scrl.clone();
    let _ = glib::idle_add_local_once(move || {
        sync_close_video_action(&cv_s2, &p_s2, &r_s2);
    });
    let close_video_rz = close_video.clone();

    let move_to_trash = gio::SimpleAction::new("move-to-trash", None);
    let p_mt = player.clone();
    let w_mt = win.clone();
    let recent_mt = recent_scrl.clone();
    let flow_mt = flow_recent.clone();
    let gl_mt = gl_area.clone();
    let op_mt = on_open.clone();
    let rem_mt = on_remove.clone();
    let trash_mt = on_trash.clone();
    let rbf_mt = recent_backfill.clone();
    let last_mt = last_path.clone();
    let seof_mt = sibling_seof.clone();
    let nav_mt = sibling_nav.clone();
    let browse_mt = browse_chrome.clone();
    let wa_mt = win_aspect.clone();
    let ush_mt = undo_shell.clone();
    let ula_mt = undo_label.clone();
    let uti_mt = undo_timer.clone();
    let ur_mt = undo_remove_stack.clone();
    let undo_b_mt = undo_btn.clone();
    let do_mt = do_commit.clone();
    move_to_trash.connect_activate(glib::clone!(
        #[strong]
        p_mt,
        #[strong]
        w_mt,
        #[strong]
        recent_mt,
        #[strong]
        flow_mt,
        #[strong]
        gl_mt,
        #[strong]
        op_mt,
        #[strong]
        rem_mt,
        #[strong]
        trash_mt,
        #[strong]
        rbf_mt,
        #[strong]
        last_mt,
        #[strong]
        seof_mt,
        #[strong]
        nav_mt,
        #[strong]
        browse_mt,
        #[strong]
        wa_mt,
        #[strong]
        ush_mt,
        #[strong]
        ula_mt,
        #[strong]
        uti_mt,
        #[strong]
        ur_mt,
        #[strong]
        undo_b_mt,
        #[strong]
        do_mt,
        move |_, _| {
            if recent_mt.is_visible() {
                return;
            }
            let path = {
                let g = p_mt.borrow();
                let Some(b) = g.as_ref() else {
                    return;
                };
                let Some(p) = local_file_from_mpv(&b.mpv) else {
                    return;
                };
                if !p.is_file() {
                    return;
                }
                p
            };
            let want = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let snap = capture_list_remove_undo(&path);
            let f = gio::File::for_path(&path);
            if let Err(e) = f.trash(gio::Cancellable::NONE) {
                eprintln!("[rhino] move to trash: {e}");
                return;
            }
            let in_trash = trash_xdg::find_trash_files_stored_path(&want);
            if in_trash.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            remove_continue_entry(&path);
            if let Some(t) = in_trash {
                ur_mt
                    .borrow_mut()
                    .push(ContinueBarUndo::Trash { snap, in_trash: t });
            }
            back_to_browse(
                &BackToBrowseCtx {
                    player: p_mt.clone(),
                    on_open: op_mt.clone(),
                    on_remove: rem_mt.clone(),
                    on_trash: trash_mt.clone(),
                    recent_backfill: rbf_mt.clone(),
                    last_path: last_mt.clone(),
                    sibling_seof: seof_mt.clone(),
                    sibling_nav: nav_mt.clone(),
                    win_aspect: wa_mt.clone(),
                    on_browse: browse_mt.clone(),
                    undo_shell: ush_mt.clone(),
                    undo_label: ula_mt.clone(),
                    undo_btn: undo_b_mt.clone(),
                    undo_timer: uti_mt.clone(),
                    undo_remove_stack: ur_mt.clone(),
                },
                &w_mt,
                &gl_mt,
                &recent_mt,
                &flow_mt,
                false,
            );
            sync_undo_bar(&ula_mt, &undo_b_mt, &ush_mt, &ur_mt);
            if !ur_mt.borrow().is_empty() {
                rearm_undo_dismiss(&do_mt, uti_mt.as_ref());
            }
        }
    ));
    app.add_action(&move_to_trash);
    *trash_action_cell.borrow_mut() = Some(move_to_trash.clone());
    let mt_s1 = move_to_trash.clone();
    let p_mt1 = player.clone();
    let r_mt1 = recent_scrl.clone();
    recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
        sync_trash_action(&mt_s1, &p_mt1, &r_mt1);
    });
    let mt_s2 = move_to_trash.clone();
    let p_mt2 = player.clone();
    let r_mt2 = recent_scrl.clone();
    let _ = glib::idle_add_local_once(move || {
        sync_trash_action(&mt_s2, &p_mt2, &r_mt2);
    });
    let move_trash_rz = move_to_trash.clone();

    VideoFileActions {
        close_video: close_video_rz,
        move_to_trash: move_trash_rz,
    }
}
