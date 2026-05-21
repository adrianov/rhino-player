// macOS programmatic fullscreen exit: reveal toolbar bars, settle, then toggleFullScreen:.
// Not set_fullscreened(false). Coalesced via macos_fs_exit; clear_exit when windowed (leave restore).

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "macos")]
static MACOS_UNFS_GEN: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL: std::time::Duration = std::time::Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL_MAX: u8 = 12;

#[cfg(target_os = "macos")]
fn macos_unfullscreen_step(win: adw::ApplicationWindow, gen: u64, retry: u8) {
    if gen != MACOS_UNFS_GEN.load(Ordering::Acquire) {
        crate::macos_fs_exit::clear_exit();
        return;
    }
    if crate::macos_window::clear_stale_gtk_fullscreen(&win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(&win) {
        crate::macos_fs_exit::clear_exit();
        return;
    }
    let gtk = win.upcast_ref::<gtk::Widget>();
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk)
        && retry < MACOS_FS_TRANSITION_POLL_MAX
    {
        let win2 = win.clone();
        let _ = glib::timeout_add_local_once(MACOS_FS_TRANSITION_POLL, move || {
            macos_unfullscreen_step(win2, gen, retry.saturating_add(1));
        });
        return;
    }
    crate::macos_fs_debug::log("toggleFullScreen");
    if !crate::macos_window::native_toggle_fullscreen_exit(&win) {
        let _ = crate::macos_window::clear_stale_gtk_fullscreen(&win);
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_schedule_unfullscreen(win: adw::ApplicationWindow) {
    if crate::macos_window::clear_stale_gtk_fullscreen(&win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(&win) {
        return;
    }
    if !crate::macos_fs_exit::try_arm_exit() {
        return;
    }
    crate::macos_window::prepare_fullscreen_exit(&win);
    macos_traffic_cancel_poll();
    let gen = MACOS_UNFS_GEN.fetch_add(1, Ordering::AcqRel) + 1;
    crate::macos_fs_debug::log_win_state("schedule_unfullscreen", &win);
    let win2 = win.clone();
    let _ = glib::timeout_add_local_once(crate::fullscreen_timing::TRANSITION_SETTLE, move || {
        macos_unfullscreen_step(win2, gen, 0);
    });
}
