/// Show the sheet immediately; save state and repaint cards after a frame while keeping the
/// current file paused as a warm reopen target when the continue strip is visible (history cards
/// and/or the Open tile on empty-history launch).
fn back_to_browse(
    c: &BackToBrowseCtx,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::Box,
    row: &gtk::Box,
    clear_undo: bool,
) {
    cancel_undo_timer(&c.undo_timer);
    if clear_undo {
        *c.undo_remove_stack.borrow_mut() = Vec::new();
    }
    sync_undo_bar(&c.undo_label, &c.undo_btn, &c.undo_shell, &c.undo_remove_stack);
    if let Some(b) = c.player.borrow().as_ref() {
        let bar = c.dvd_bar.borrow();
        b.save_playback_state_for_close_with_bar(bar.as_ref());
    }
    c.playback_focus.set(false);
    c.win_aspect.set(None);
    c.sibling_seof.done.set(false);
    // Keep last_path set to the warm preload target so prev/next remain active
    // on the browse screen and the sibling nav works immediately after warm resume.
    let warm_path = c.player.borrow().as_ref().and_then(|b| {
        crate::media_probe::shell_media_path(&b.mpv, b.me_budget_shell_path.borrow().as_deref())
    });
    *c.last_path.borrow_mut() = warm_path.clone();
    c.sibling_nav.refresh(warm_path.as_deref(), &c.sibling_seof);
    let paths: Vec<PathBuf> = history::load()
        .into_iter()
        .take(crate::recent_view::CONTINUE_DISPLAY_MAX)
        .collect();
    let show_strip = !paths.is_empty() || c.browse_has_strip;
    recent.set_visible(show_strip);
    (c.on_browse)();
    sync_app_window_title(win, c.hdr_title_mirror.as_deref(), Some(APP_WIN_TITLE));
    gl.queue_render();
    // Cut audio right away; `stop` stays in idlers so a last-frame screenshot can run first.
    c.recent_visible.set(recent.is_visible());
    if let Some(b) = c.player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }

    if !show_strip {
        let p2 = c.player.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(b) = p2.borrow().as_ref() {
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        schedule_sync_close_video_idle(c, recent);
        return;
    }

    // FnOnce chain: `idle_add_local_full` requires FnMut, so the grid refill is scheduled from
    // a one-shot idle (paint can run first at DEFAULT_IDLE priority).
    let row2 = row.clone();
    let op2 = c.on_open.clone();
    let osl2 = c.on_remove.clone();
    let otr2 = c.on_trash.clone();
    let paths2 = paths;
    let rbb = c.recent_backfill.clone();
    let chrome_cache = Rc::clone(&c.continue_grid_cache);
    let _ = glib::source::idle_add_local_once(move || {
        let rbb2 = rbb.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            let warm = rbb2
                .borrow()
                .as_ref()
                .and_then(|c| c.warm_hover().cloned());
            recent_view::fill_row(
                &row2,
                v,
                op2.clone(),
                osl2.clone(),
                otr2.clone(),
                warm.as_ref(),
                Some(&chrome_cache),
            );
            let warm_ctx = rbb2.borrow().as_ref().and_then(|c| c.warm_hover().cloned());
            let n = recent_view::ensure_recent_backfill(
                &rbb2,
                &row2,
                op2.clone(),
                osl2.clone(),
                otr2.clone(),
                warm_ctx,
                Rc::clone(&chrome_cache),
            );
            recent_view::schedule_thumb_backfill(n, paths2.clone());
            glib::ControlFlow::Break
        });
    });
    schedule_sync_close_video_idle(c, recent);
}

/// Wraps [back_to_browse] into a single `Rc<dyn Fn(bool)>` closure (arg = `clear_undo`).
/// Build once in `build_window`; pass to every call site instead of repeating [BackToBrowseCtx].
fn make_browse_back(
    ctx: BackToBrowseCtx,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    row: gtk::Box,
) -> Rc<dyn Fn(bool)> {
    Rc::new(move |clear_undo| {
        back_to_browse(&ctx, &win, &gl, &recent, &row, clear_undo);
    })
}
