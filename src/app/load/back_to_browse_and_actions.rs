/// Show the sheet immediately; save state and repaint cards after a frame while keeping the
/// current file paused as a warm reopen target when the continue list is non-empty.
fn back_to_browse(
    c: &BackToBrowseCtx,
    win: &impl IsA<gtk::Window>,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    row: &gtk::Box,
    clear_undo: bool,
) {
    cancel_undo_timer(&c.undo_timer);
    if clear_undo {
        *c.undo_remove_stack.borrow_mut() = Vec::new();
        sync_undo_bar(
            &c.undo_label,
            &c.undo_btn,
            &c.undo_shell,
            &c.undo_remove_stack,
        );
    }
    c.win_aspect.set(None);
    *c.last_path.borrow_mut() = None;
    c.sibling_seof.done.set(false);
    c.sibling_nav.set_no_media();
    let paths: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if paths.is_empty() {
        recent.set_visible(false);
    } else {
        recent.set_visible(true);
    }
    (c.on_browse)();
    win.upcast_ref::<gtk::Window>()
        .set_title(Some(APP_WIN_TITLE));
    gl.queue_render();
    // Cut audio right away; `stop` stays in idlers so a last-frame screenshot can run first.
    if let Some(b) = c.player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }

    if paths.is_empty() {
        let p2 = c.player.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(b) = p2.borrow().as_ref() {
                b.snapshot_outgoing_before_leave();
                b.save_playback_state();
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        return;
    }

    // FnOnce chain: `idle_add_local_full` requires FnMut, so the grid refill is scheduled from
    // a one-shot idle (paint can run first at DEFAULT_IDLE priority).
    let p_write = c.player.clone();
    let row2 = row.clone();
    let op2 = c.on_open.clone();
    let osl2 = c.on_remove.clone();
    let otr2 = c.on_trash.clone();
    let paths2 = paths;
    let rbb = c.recent_backfill.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if let Some(b) = p_write.borrow().as_ref() {
            b.snapshot_outgoing_before_leave();
        }
        let rbb2 = rbb.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            recent_view::fill_row(&row2, v, op2.clone(), osl2.clone(), otr2.clone());
            let n = recent_view::ensure_recent_backfill(
                &rbb2,
                &row2,
                op2.clone(),
                osl2.clone(),
                otr2.clone(),
            );
            recent_view::schedule_thumb_backfill(n, paths2.clone());
            glib::ControlFlow::Break
        });
    });
}

/// Enables [gio::SimpleAction] `app.close-video` when the player is ready and the continue grid is hidden.
fn sync_close_video_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    a.set_enabled(player.borrow().is_some() && !recent.is_visible());
}

/// Enables [gio::SimpleAction] `app.move-to-trash` for a local file in playback (not streams / empty path).
fn sync_trash_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        a.set_enabled(false);
        return;
    };
    let ok = !recent.is_visible() && local_file_from_mpv(&b.mpv).is_some_and(|p| p.is_file());
    a.set_enabled(ok);
}

/// Hides the window, then (after GTK can draw the hide) saves DB resume, stops, and quits.
fn schedule_quit_persist(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
) {
    win.set_visible(false);
    let p = player.clone();
    let a = app.clone();
    let sp = Rc::clone(sub);
    let ic = Rc::clone(idle_inhib);
    let _ = glib::idle_add_local(move || {
        idle_inhibit::clear(&a, &ic);
        if let Some(b) = p.borrow().as_ref() {
            save_mpv_state(&b.mpv, &sp);
            b.commit_quit();
        }
        a.quit();
        glib::ControlFlow::Break
    });
}
