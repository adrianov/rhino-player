/// Whether AppKit reports this window in native fullscreen (style mask).
pub(crate) fn ns_window_is_native_fullscreen(nswin: &NSWindow) -> bool {
    nswin.styleMask().contains(NSWindowStyleMask::FullScreen)
}

pub(crate) fn ns_fullscreen_for_win(win: &adw::ApplicationWindow) -> bool {
    nswindow_for_widget(win.upcast_ref::<gtk::Widget>())
        .is_some_and(|ns| ns_window_is_native_fullscreen(&ns))
}

/// GDK `is_fullscreen` without a matching AppKit style mask (maximized-looking stuck state).
pub(crate) fn clear_stale_gtk_fullscreen(win: &adw::ApplicationWindow) -> bool {
    if !win.is_fullscreen() || ns_fullscreen_for_win(win) {
        return false;
    }
    crate::macos_fs_debug::log("clear stale gtk fullscreen (ns not fullscreen)");
    win.set_fullscreened(false);
    crate::macos_fs_exit::clear_exit();
    true
}

/// AppKit native fullscreen is authoritative (GDK `is_fullscreen` can lag or stick after exit).
pub(crate) fn window_still_fullscreen(win: &adw::ApplicationWindow) -> bool {
    ns_fullscreen_for_win(win)
}

/// Whether GDK's [`GdkMacosWindow`] is inside AppKit's fullscreen enter/exit animation.
pub(crate) fn gdk_macos_in_fullscreen_transition<W: IsA<gtk::Widget>>(widget: &W) -> bool {
    let Some(nswin) = nswindow_for_widget(widget) else {
        return false;
    };
    unsafe { msg_send![&*nswin, inFullscreenTransition] }
}

/// Enter native fullscreen from maximized (or windowed); fall back to GTK if toggle is unavailable.
pub(crate) fn enter_fullscreen_from_maximized(win: &adw::ApplicationWindow) {
    if !native_toggle_fullscreen_enter(win) {
        win.fullscreen();
    }
}

include!("macos_window_fs_layout_guard.rs");

/// GDK-style guarded `toggleFullScreen:` to enter native fullscreen from maximized/windowed.
pub(crate) fn native_toggle_fullscreen_enter(win: &adw::ApplicationWindow) -> bool {
    let gtk = win.upcast_ref::<gtk::Widget>();
    let Some(nswin) = nswindow_for_widget(gtk) else {
        return false;
    };
    if gdk_macos_in_fullscreen_transition(gtk) || ns_window_is_native_fullscreen(&nswin) {
        return false;
    }
    // Live before any exit path (system chrome included), not only our prep.
    ensure_titlebar_layout_guard();
    unsafe {
        let _: () = msg_send![&*nswin, toggleFullScreen: &*nswin];
    }
    true
}

/// Flatten AppKit titlebar state before native fullscreen **exit**.
///
/// `_NSExitFullScreenTransitionController prepareToExitFullScreenMode` calls
/// `setCustomTitlebarHeight:`, which on macOS 26.x can recurse
/// `_updateTitlebarContainerViewFrameIfNecessary` ↔ `_syncToolbarPosition` until
/// stack overflow. The `_syncToolbarPosition` reentrancy guard (installed on enter
/// and here) cuts that loop. Hide lights and drop the CSD `NSToolbar` before
/// `toggleFullScreen:` so zoom-cell tracking is quiet during the transition.
pub(crate) fn prep_native_fullscreen_exit(nswin: &NSWindow) {
    use objc2::runtime::AnyObject;

    ensure_titlebar_layout_guard();
    flatten_traffic_lights(nswin);
    unsafe {
        let none: Option<&AnyObject> = None;
        let _: () = msg_send![nswin, setToolbar: none];
    }
    crate::macos_fs_debug::log("prep native fullscreen exit (titlebar flattened)");
}

/// macOS 26's native-exit machinery re-assigns an empty `NSToolbar` after our prep dropped
/// it (gdk-macos assigns one to indent the traffic lights and never clears it again). The
/// stale toolbar leaves the lights ~20px right of their normal slot until the next legacy
/// fullscreen exit re-lays them. Drop it and let AppKit re-layout to the standard slot.
pub(crate) fn clear_stale_titlebar_toolbar(nswin: &NSWindow) {
    use objc2::runtime::AnyObject;

    if nswin.toolbar().is_none() {
        return;
    }
    unsafe {
        let none: Option<&AnyObject> = None;
        let _: () = msg_send![nswin, setToolbar: none];
    }
}

/// Schedule a stale-toolbar clear for when the native-exit animation has fully finished —
/// macOS 26 assigns the toolbar during exit completion, after [`prep_native_fullscreen_exit`]
/// already dropped it, so the immediate clear is not enough.
pub(crate) fn schedule_titlebar_toolbar_clear(win: &adw::ApplicationWindow) {
    let w2 = win.clone();
    let _ = glib::timeout_add_local_once(
        crate::fullscreen_timing::TRANSITION_SETTLE * 2,
        move || {
            if crate::macos_fs_exit::exit_armed()
                || crate::macos_window::window_still_fullscreen(&w2)
            {
                return;
            }
            if let Some(nswin) = nswindow_for_widget(&w2) {
                clear_stale_titlebar_toolbar(&nswin);
            }
        },
    );
}
