// Availability and tooltips for Close Video / Move to Trash, plus the quit persistence chain.

#[cfg(target_os = "macos")]
const CLOSE_VIDEO_PLAYBACK_TIP: &str = "Close Video (Cmd+W)";
#[cfg(not(target_os = "macos"))]
const CLOSE_VIDEO_PLAYBACK_TIP: &str = "Close Video (Ctrl+W)";

/// True when a local file or Blu-ray disc tree is loaded (warm preload, playing, or paused behind
/// the grid). Existence on disk is deliberately not consulted: a file renamed or deleted
/// mid-playback still fills the window, so Close returns to browse rather than quitting.
pub(crate) fn has_loaded_local_media(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    player.borrow().as_ref().is_some_and(|b| {
        crate::media_probe::open_media_path(&b.mpv, b.me_budget_shell_path.borrow().as_deref())
            .is_some()
    })
}

/// Enables `app.close-video` and matches the bottom close button tooltip to browse vs playback.
fn sync_close_video_action(
    a: &gio::SimpleAction,
    tip_target: &gtk::Button,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    let has_player = player.borrow().is_some();
    let grid = recent.is_visible();
    a.set_enabled(has_player || grid);

    let tip = if grid || !has_loaded_local_media(player) {
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
    let recent = recent.clone();
    let _ = glib::idle_add_local_once(move || {
        let g = cell.borrow();
        let Some(a) = g.as_ref() else {
            return;
        };
        sync_close_video_action(a, &tip_target, &p, &recent);
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
    idle_inhib: &Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
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
