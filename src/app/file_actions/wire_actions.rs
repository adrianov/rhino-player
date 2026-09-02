include!("close_video_action.rs");

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
        close_video_btn,
        ..
    } = ctx;

    let close_video = install_close_video_action(
        &app,
        &player,
        &recent_scrl,
        &on_browse_back,
        &close_video_btn,
        &close_action_cell,
    );

    let undo_deps = TrashUndoDeps {
        player: &player,
        recent_scrl: &recent_scrl,
        undo_remove_stack: &undo_remove_stack,
        undo_timer: &undo_timer,
        do_commit: &do_commit,
        on_browse_back: &on_browse_back,
    };
    let move_to_trash = install_move_to_trash_action(&app, &undo_deps, &trash_action_cell);

    VideoFileActions {
        close_video,
        move_to_trash: move_to_trash.clone(),
    }
}

fn install_close_video_action(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_scrl: &gtk::Box,
    on_browse_back: &Rc<dyn Fn(bool)>,
    close_video_btn: &gtk::Button,
    close_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
) -> gio::SimpleAction {
    let close_video = make_close_video_action(app, player, recent_scrl, on_browse_back);
    app.add_action(&close_video);
    *close_action_cell.borrow_mut() = Some(close_video.clone());
    wire_close_video_visible_sync(recent_scrl, &close_video, player, close_video_btn);
    close_video
}

/// Handles shared by the move-to-trash flow: the loaded file, the recent list it is
/// removed from, the undo stack/timer, and commit + browse-back hooks.
struct TrashUndoDeps<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    recent_scrl: &'a gtk::Box,
    undo_remove_stack: &'a Rc<RefCell<Vec<ContinueBarUndo>>>,
    undo_timer: &'a Rc<RefCell<Option<glib::source::SourceId>>>,
    do_commit: &'a Rc<dyn Fn() + 'static>,
    on_browse_back: &'a Rc<dyn Fn(bool)>,
}

fn install_move_to_trash_action(
    app: &adw::Application,
    d: &TrashUndoDeps,
    trash_action_cell: &Rc<RefCell<Option<gio::SimpleAction>>>,
) -> gio::SimpleAction {
    let move_to_trash = make_move_to_trash_action(d);
    app.add_action(&move_to_trash);
    *trash_action_cell.borrow_mut() = Some(move_to_trash.clone());
    wire_trash_visible_sync(d.recent_scrl, &move_to_trash, d.player);
    move_to_trash
}

fn make_move_to_trash_action(d: &TrashUndoDeps) -> gio::SimpleAction {
    let move_to_trash = gio::SimpleAction::new("move-to-trash", None);
    {
        let p = d.player.clone();
        let r = d.recent_scrl.clone();
        let ur = d.undo_remove_stack.clone();
        let ut = d.undo_timer.clone();
        let dc = d.do_commit.clone();
        let bb = d.on_browse_back.clone();
        move_to_trash.connect_activate(move |_, _| {
            if r.is_visible() {
                return;
            }
            crate::user_action_log::act("move to trash (playing file)");
            let Some(path) = playing_local_file_for_trash(&p) else {
                return;
            };
            commit_move_to_trash(&path, &p, &ur, &ut, &dc, &bb);
        });
    }
    move_to_trash
}

/// The local file currently loaded in mpv, if it still exists on disk.
fn playing_local_file_for_trash(p: &Rc<RefCell<Option<MpvBundle>>>) -> Option<std::path::PathBuf> {
    let g = p.borrow();
    let b = g.as_ref()?;
    let p = local_file_from_mpv(&b.mpv)?;
    if !p.is_file() {
        return None;
    }
    Some(p)
}

fn commit_move_to_trash(
    path: &std::path::Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    ur: &Rc<RefCell<Vec<ContinueBarUndo>>>,
    ut: &Rc<RefCell<Option<glib::source::SourceId>>>,
    dc: &Rc<dyn Fn() + 'static>,
    bb: &Rc<dyn Fn(bool)>,
) {
    let snap = capture_list_remove_undo(path);
    set_persist_skip(player, true);
    let Some(in_trash) = trash_playing_file(path) else {
        set_persist_skip(player, false);
        return;
    };
    if let Some(b) = player.borrow().as_ref() {
        b.stop_playback();
    }
    finish_playing_trash(snap, in_trash, ur, ut, dc, bb);
}

fn set_persist_skip(player: &Rc<RefCell<Option<MpvBundle>>>, skip: bool) {
    if let Some(b) = player.borrow().as_ref() {
        b.set_skip_media_persist(skip);
    }
}

fn trash_playing_file(path: &std::path::Path) -> Option<Option<std::path::PathBuf>> {
    match trash_xdg::trash_local_file_for_undo(path) {
        Err(e) => {
            eprintln!("[rhino] move to trash: {e}");
            None
        }
        Ok(loc) => {
            if loc.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            Some(loc)
        }
    }
}

fn finish_playing_trash(
    snap: crate::media_probe::ListRemoveUndo,
    in_trash: Option<std::path::PathBuf>,
    ur: &Rc<RefCell<Vec<ContinueBarUndo>>>,
    ut: &Rc<RefCell<Option<glib::source::SourceId>>>,
    dc: &Rc<dyn Fn() + 'static>,
    bb: &Rc<dyn Fn(bool)>,
) {
    let key = snap.path.clone();
    remove_continue_entry(&key);
    crate::recent_view::note_path_trashed(&key);
    crate::db::forget_file(&key);
    if let Some(t) = in_trash {
        ur.borrow_mut()
            .push(ContinueBarUndo::Trash { snap, in_trash: t });
    }
    bb(false);
    if !ur.borrow().is_empty() {
        rearm_undo_dismiss(dc, ut);
    }
}

fn wire_trash_visible_sync(
    recent_scrl: &gtk::Box,
    move_to_trash: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
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
}
