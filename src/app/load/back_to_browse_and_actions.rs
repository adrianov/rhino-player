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
    c.playback_focus.set(false);
    if clear_undo {
        *c.undo_remove_stack.borrow_mut() = Vec::new();
    }
    sync_undo_bar(&c.undo_label, &c.undo_btn, &c.undo_shell, &c.undo_remove_stack);
    c.win_aspect.set(None);
    c.sibling_seof.done.set(false);
    // Keep last_path set to the warm preload target so prev/next remain active
    // on the browse screen and the sibling nav works immediately after warm resume.
    let warm_path = c.player.borrow().as_ref()
        .and_then(|b| local_file_from_mpv(&b.mpv))
        .and_then(|p| std::fs::canonicalize(&p).ok());
    *c.last_path.borrow_mut() = warm_path.clone();
    c.sibling_nav.refresh(warm_path.as_deref(), &c.sibling_seof);
    let paths: Vec<PathBuf> = history::load().into_iter().take(5).collect();
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
                b.snapshot_outgoing_before_leave();
                b.save_playback_state();
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        schedule_sync_close_video_idle(c, recent);
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

#[cfg(target_os = "macos")]
const CLOSE_VIDEO_PLAYBACK_TIP: &str = "Close Video (Cmd+W)";
#[cfg(not(target_os = "macos"))]
const CLOSE_VIDEO_PLAYBACK_TIP: &str = "Close Video (Ctrl+W)";

/// Enables `app.close-video` and matches the bottom close button tooltip to browse vs playback.
fn sync_close_video_action(
    a: &gio::SimpleAction,
    tip_target: &gtk::Button,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
    playback_focus: &Cell<bool>,
) {
    let has_player = player.borrow().is_some();
    let grid = recent.is_visible();
    a.set_enabled(has_player || grid);

    let tip = if grid || !playback_focus.get() {
        "Quit Rhino Player"
    } else {
        CLOSE_VIDEO_PLAYBACK_TIP
    };
    if tip_target.tooltip_text().as_deref() != Some(tip) {
        tip_target.set_tooltip_text(Some(tip));
    }
}

fn schedule_sync_close_video_idle(c: &BackToBrowseCtx, recent: &gtk::Box) {
    let cell = Rc::clone(&c.close_action_cell);
    let tip_target = c.close_video_btn.clone();
    let p = c.player.clone();
    let pf = Rc::clone(&c.playback_focus);
    let recent = recent.clone();
    let _ = glib::idle_add_local_once(move || {
        let g = cell.borrow();
        let Some(a) = g.as_ref() else {
            return;
        };
        sync_close_video_action(a, &tip_target, &p, &recent, pf.as_ref());
    });
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

/// Saves DB resume and stops playback from an idle, then runs [`MpvBundle::teardown_gl_paint`] on the
/// next [`gtk::GLArea::render`] after [`gtk::GLArea::queue_render`], then an idle that **binds that
/// `GLArea`’s GL context** before [`MpvBundle::dispose_for_quit`] (frees render context + `mpv_terminate_destroy`).
///
/// Teardown must not nest inside GTK snapshot repaint; `mpv_destroy` from the Rust wrapper’s `Drop`
/// path aborts on macOS when `vo=libmpv` is still active.
fn schedule_quit_persist(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
    teardown_after_draw: &Rc<Cell<bool>>,
) {
    let p = player.clone();
    let a = app.clone();
    let w = win.clone();
    let sp = Rc::clone(sub);
    let ic = Rc::clone(idle_inhib);
    let gl = gl.clone();
    let td = Rc::clone(teardown_after_draw);
    let _ = glib::idle_add_local(move || {
        idle_inhibit::clear(&a, &ic);
        #[cfg(target_os = "macos")]
        crate::macos_window::set_system_cursor_hidden(false);
        if let Some(b) = p.borrow().as_ref() {
            save_mpv_state(&b.mpv, &sp);
            b.commit_quit();
        }
        // Map once if needed (`queue_render` no-ops until realized). Calling `present`/`realize`
        // redundantly on macOS can disturb CvDisplayLink while tearing down during quit-from-pause.
        if !w.is_visible() {
            w.present();
        }
        if !gl.is_realized() {
            gl.realize();
        }
        td.set(true);
        gl.queue_render();
        #[cfg(not(target_os = "macos"))]
        gl.queue_draw();
        glib::ControlFlow::Break
    });
}
