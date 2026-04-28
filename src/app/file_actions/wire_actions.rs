fn wire_video_file_actions(ctx: VideoFileActionCtx) -> VideoFileActions {
    let VideoFileActionCtx {
        app,
        player,
        recent: recent_scrl,
        on_browse_back,
        undo_timer,
        undo_remove_stack,
        do_commit,
        close_action_cell,
        trash_action_cell,
    } = ctx;

    let close_video = gio::SimpleAction::new("close-video", None);
    {
        let p = player.clone();
        let r = recent_scrl.clone();
        let bb = on_browse_back.clone();
        close_video.connect_activate(move |_, _| {
            if r.is_visible() || p.borrow().is_none() {
                return;
            }
            bb(true);
        });
    }
    app.add_action(&close_video);
    *close_action_cell.borrow_mut() = Some(close_video.clone());
    {
        let cv = close_video.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
            sync_close_video_action(&cv, &p, &r);
        });
    }
    let _ = glib::idle_add_local_once({
        let cv = close_video.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        move || sync_close_video_action(&cv, &p, &r)
    });
    let close_video_rz = close_video.clone();

    let move_to_trash = gio::SimpleAction::new("move-to-trash", None);
    {
        let p = player.clone();
        let r = recent_scrl.clone();
        let ur = undo_remove_stack.clone();
        let ut = undo_timer.clone();
        let dc = do_commit.clone();
        let bb = on_browse_back.clone();
        move_to_trash.connect_activate(move |_, _| {
            if r.is_visible() {
                return;
            }
            let path = {
                let g = p.borrow();
                let Some(b) = g.as_ref() else { return };
                let Some(p) = local_file_from_mpv(&b.mpv) else { return };
                if !p.is_file() { return; }
                p
            };
            let want = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let snap = capture_list_remove_undo(&path);
            if let Err(e) = gio::File::for_path(&path).trash(gio::Cancellable::NONE) {
                eprintln!("[rhino] move to trash: {e}");
                return;
            }
            let in_trash = trash_xdg::find_trash_files_stored_path(&want);
            if in_trash.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            remove_continue_entry(&path);
            if let Some(t) = in_trash {
                ur.borrow_mut().push(ContinueBarUndo::Trash { snap, in_trash: t });
            }
            // back_to_browse syncs the undo bar after the push above.
            bb(false);
            if !ur.borrow().is_empty() {
                rearm_undo_dismiss(&dc, ut.as_ref());
            }
        });
    }
    app.add_action(&move_to_trash);
    *trash_action_cell.borrow_mut() = Some(move_to_trash.clone());
    {
        let mt = move_to_trash.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
            sync_trash_action(&mt, &p, &r);
        });
    }
    let _ = glib::idle_add_local_once({
        let mt = move_to_trash.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        move || sync_trash_action(&mt, &p, &r)
    });

    VideoFileActions {
        close_video: close_video_rz,
        move_to_trash: move_to_trash.clone(),
    }
}
