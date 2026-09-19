/// Force gdk-macos to rebuild its `GdkMacosTile` sublayers in fullscreen states: a
/// micro-resize re-tiles the surface backing, replacing tiles whose frame/contentsRect
/// went stale (they display the titlebar strip stretched mid-screen — the header band).
/// [`nudge_gdk_compositing_width`] skips fullscreen/maximized windows by design; this
/// variant is for exactly those states, so it takes no guard and restores the frame
/// instead of touching `default_size` (the cover/restore machinery owns it there).
///
/// The size is captured from the NSWindow frame, not GTK width/height: at activation the
/// maximized-residue latch can transiently render windowed geometry, and a restore captured
/// from that transient would fight the cover reassert. Nudging and restoring the frame the
/// NSWindow actually has keeps the operation self-consistent in any transient state; if the
/// session exits during the one-tick window, the restore is skipped and the exit path owns
/// the geometry.
pub fn retile_gdk_compositing_fullscreen(win: &adw::ApplicationWindow) {
    let Some((w, h)) = retile_target_frame(win) else {
        return;
    };
    crate::app::note_programmatic_win_resize(w, h);
    force_nswindow_frame(win, w + 1, h);
    invalidate_window_layers(win);
    let win2 = win.clone();
    let _ = glib::idle_add_local_once(move || {
        if !crate::macos_legacy_fs::active() && !win2.is_fullscreen() {
            // Always-on: a skipped restore leaves the +1 frame until the exit path resets it;
            // without this line that state is indistinguishable from a wedged heal.
            eprintln!("[rhino] retile: restore skipped — session exited during tick");
            return;
        }
        retile_restore_pass(win2, w, h);
    });
}

/// Capture the NSWindow frame to re-tile around. Always-on skip lines: the heal silently
/// not running is exactly what a band recurrence looks like.
fn retile_target_frame(win: &adw::ApplicationWindow) -> Option<(i32, i32)> {
    let Some(nswin) = nswindow_for_widget(win) else {
        eprintln!("[rhino] retile: skip — no NSWindow for the application window");
        return None;
    };
    let frame = nswin.frame();
    let (w, h) = (frame.size.width as i32, frame.size.height as i32);
    if w < 322 || h < 200 {
        eprintln!("[rhino] retile: skip — frame {w}x{h} outside re-tile bounds");
        return None;
    }
    Some((w, h))
}

/// Second half of [`retile_gdk_compositing_fullscreen`]: land the micro-resize restore.
fn retile_restore_pass(win: adw::ApplicationWindow, w: i32, h: i32) {
    crate::app::note_programmatic_win_resize(w, h);
    force_nswindow_frame(&win, w, h);
    request_gdk_surface_layout(&win);
    invalidate_window_layers(&win);
    crate::shell_debug_log::log(format!("fullscreen retile restore {w}x{h}"));
}
