include!("chrome_fs_transition_gate.rs");
include!("chrome_macos_unfullscreen_defer.rs");
#[cfg(target_os = "macos")]
include!("chrome_macos_toggle.rs");
#[cfg(target_os = "macos")]
include!("chrome_macos_header_popovers.rs");
include!("chrome_header_menubtns.rs");

/// Refs shared by menu / gesture fullscreen toggles (one bundle keeps wiring arity small).
struct FullscreenToggleRefs {
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    fs_transition_busy: Rc<Cell<bool>>,
}

#[cfg(not(target_os = "macos"))]
fn unfullscreen_safe_inner(win: &adw::ApplicationWindow) {
    win.unfullscreen();
}

/// Linux demaximize path: synchronous [`GtkWindowExt::unfullscreen`] after [`fs_transition_try_begin`].
#[cfg(not(target_os = "macos"))]
fn unfullscreen_safe(win: &adw::ApplicationWindow, fs_busy: &Cell<bool>) {
    if !fs_transition_try_begin(fs_busy) {
        return;
    }
    unfullscreen_safe_inner(win);
}

#[cfg(target_os = "macos")]
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
    _fs_busy: &Cell<bool>,
) {
    macos_toggle_fullscreen(win, fs_restore, last_unmax, skip_max_to_fs);
}

#[cfg(not(target_os = "macos"))]
fn maximize_then_fullscreen_later(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
) {
    *fs_restore.borrow_mut() = Some(win_normal_size(win));
    win.maximize();
}

#[cfg(not(target_os = "macos"))]
fn fullscreen_from_maximized(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
) {
    if fs_restore.borrow().is_none() {
        *fs_restore.borrow_mut() = Some(*last_unmax.borrow());
    }
    win.fullscreen();
}

#[cfg(not(target_os = "macos"))]
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
    fs_busy: &Cell<bool>,
) {
    if !fs_transition_try_begin(fs_busy) {
        return;
    }
    if win.is_fullscreen() {
        skip_max_to_fs.set(true);
        unfullscreen_safe_inner(win);
    } else if !win.is_maximized() {
        maximize_then_fullscreen_later(win, fs_restore);
    } else {
        fullscreen_from_maximized(win, fs_restore, last_unmax);
    }
}

include!("chrome_header_csd_controls.rs");
include!("chrome_pointer_after_bars.rs");
include!("chrome_apply.rs");

/// Clicks to another header [gtk::MenuButton] are blocked while a **modal** popover is open.
/// [gtk::Popover:modal] on GTK 4.14+ — set to false so the rest of the window (including
/// the other header buttons) stays clickable.
/// Linux: [gtk::Popover:autohide] still dismisses on outside press.
/// macOS: autohide off — opening click would dismiss immediately; use capture dismiss instead.
fn header_popover_non_modal(pop: &impl IsA<gtk::Popover>) {
    use glib::prelude::Cast;
    let pop = pop.upcast_ref::<gtk::Popover>();
    if pop.find_property("modal").is_some() {
        pop.set_property("modal", false);
    }
    #[cfg(target_os = "macos")]
    pop.set_autohide(false);
}

fn video_dim_pair(mpv: &Mpv, wk: &str, hk: &str) -> Option<(i64, i64)> {
    let w = mpv.get_property::<i64>(wk).ok()?;
    let h = mpv.get_property::<i64>(hk).ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

/// Display (or stream) size in pixels from mpv, if known.
fn video_display_dims(mpv: &Mpv) -> Option<(i64, i64)> {
    video_dim_pair(mpv, "dwidth", "dheight").or_else(|| video_dim_pair(mpv, "width", "height"))
}

/// Coded stream size for post-resize aspect snap (stable when a `vf` changes `dwidth`/`dheight`).
fn video_snap_aspect_dims(mpv: &Mpv) -> Option<(i64, i64)> {
    video_dim_pair(mpv, "width", "height").or_else(|| video_dim_pair(mpv, "dwidth", "dheight"))
}

fn window_size_for_horizontal_video(vw: i64, vh: i64) -> (i32, i32) {
    let wf = vw as f64;
    let hf = vh as f64;
    let mut nw = FIT_H_VIDEO_W;
    let mut nh = (FIT_H_VIDEO_W as f64 * hf / wf).round() as i32;
    if nh > FIT_H_VIDEO_MAX_H {
        nh = FIT_H_VIDEO_MAX_H;
        nw = (FIT_H_VIDEO_MAX_H as f64 * wf / hf).round() as i32;
    }
    nw = nw.clamp(320, 4096);
    nh = nh.clamp(200, 4096);
    (nw, nh)
}

include!("chrome_shell_layout.rs");
include!("chrome_window_video_fit.rs");
include!("chrome_menu_wire.rs");

include!("chrome_subtitle_button_scan.rs");

fn drain_recent_backfill(pending: &Rc<RefCell<Option<RecentBackfillJob>>>) {
    if let Some((ctx, paths)) = pending.borrow_mut().take() {
        recent_view::schedule_thumb_backfill(ctx, paths);
    }
}
