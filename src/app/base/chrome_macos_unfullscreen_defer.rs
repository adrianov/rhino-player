// macOS programmatic fullscreen exit: arm guard, hide bars, flatten titlebar, hop to libdispatch,
// then toggleFullScreen:. Do not reveal ToolbarView bars while armed (titlebar layout recursion).
// Not set_fullscreened(false). Coalesced via macos_fs_exit; clear_exit when windowed (leave restore).

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL: std::time::Duration = std::time::Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_FS_TRANSITION_POLL_MAX: u8 = 12;

// libdispatch hop lives in the sibling unit included below.
include!("chrome_macos_exit_dispatch.rs");

/// Transition still in progress: re-poll, or give up after the capped retries.
#[cfg(target_os = "macos")]
fn macos_rearm_step_after_transition(win: adw::ApplicationWindow, gen: u64, retry: u8) {
    if retry < MACOS_FS_TRANSITION_POLL_MAX {
        let win2 = win.clone();
        let _ = glib::timeout_add_local_once(MACOS_FS_TRANSITION_POLL, move || {
            macos_unfullscreen_step(win2, gen, retry.saturating_add(1));
        });
        return;
    }
    eprintln!(
        "[rhino] macos-fs: exit gave up — still inFullscreenTransition gen={gen} retry={retry}"
    );
    crate::macos_fs_exit::clear_exit();
}

/// Dispatch schedule failed — force the stale-fullscreen clear and disarm.
#[cfg(target_os = "macos")]
fn abort_dispatch_exit(win: &adw::ApplicationWindow, gen: u64, retry: u8) {
    eprintln!(
        "[rhino] macos-fs: dispatch exit schedule failed gen={gen} retry={retry} gtk={} ns={}",
        win.is_fullscreen(),
        crate::macos_window::ns_fullscreen_for_win(win),
    );
    let _ = crate::macos_window::clear_stale_gtk_fullscreen(win);
    crate::macos_fs_exit::clear_exit();
}

#[cfg(target_os = "macos")]
fn macos_unfullscreen_step(win: adw::ApplicationWindow, gen: u64, retry: u8) {
    // Stale gen: a newer arm owns exit_armed — do not clear it.
    if !crate::macos_fs_exit::unfs_gen_is_current(gen) {
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
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk) {
        macos_rearm_step_after_transition(win, gen, retry);
        return;
    }
    if !macos_dispatch_toggle_exit(&win, gen, retry) {
        abort_dispatch_exit(&win, gen, retry);
    }
}

#[cfg(target_os = "macos")]
fn macos_hide_bars_for_exit(win: &adw::ApplicationWindow) {
    use gtk::prelude::Cast;

    let Some(ovl) = win
        .content()
        .and_then(|c| c.downcast::<gtk::Overlay>().ok())
    else {
        eprintln!("[rhino] macos-fs: exit hide bars: no overlay content");
        return;
    };
    let Some(root) = ovl
        .child()
        .and_then(|c| c.downcast::<adw::ToolbarView>().ok())
    else {
        eprintln!("[rhino] macos-fs: exit hide bars: no ToolbarView");
        return;
    };
    let _ = set_toolbar_reveal(&root, false);
}

#[cfg(target_os = "macos")]
pub(super) fn macos_schedule_unfullscreen(win: adw::ApplicationWindow) {
    if crate::macos_window::clear_stale_gtk_fullscreen(&win) {
        return;
    }
    if !crate::macos_window::window_still_fullscreen(&win) {
        return;
    }
    let Some(gen) = crate::macos_fs_exit::try_arm_exit() else {
        return;
    };
    macos_traffic_cancel_poll();
    macos_hide_bars_for_exit(&win);
    if let Some(nswin) = crate::macos_window::nswindow_for_widget(&win) {
        crate::macos_window::prep_native_fullscreen_exit(&nswin);
    }
    crate::macos_fs_debug::log_win_state("schedule_unfullscreen", &win);
    let win2 = win.clone();
    // Settle past the click/gesture layout burst before hopping to libdispatch.
    let _ = glib::timeout_add_local_once(crate::fullscreen_timing::TRANSITION_SETTLE, move || {
        macos_unfullscreen_step(win2, gen, 0);
    });
}
