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

/// GDK-style guarded `toggleFullScreen:` to enter native fullscreen from maximized/windowed.
pub(crate) fn native_toggle_fullscreen_enter(win: &adw::ApplicationWindow) -> bool {
    let gtk = win.upcast_ref::<gtk::Widget>();
    let Some(nswin) = nswindow_for_widget(gtk) else {
        return false;
    };
    if gdk_macos_in_fullscreen_transition(gtk) || ns_window_is_native_fullscreen(&nswin) {
        return false;
    }
    unsafe {
        let _: () = msg_send![&*nswin, toggleFullScreen: &*nswin];
    }
    true
}

/// Flatten AppKit titlebar state before native fullscreen **exit**.
///
/// `_NSExitFullScreenTransitionController prepareToExitFullScreenMode` calls
/// `setCustomTitlebarHeight:`. With CSD chrome / traffic-light zoom-cell tracking still
/// live, that recurses `_updateTitlebarContainerViewFrameIfNecessary` ↔
/// `_syncToolbarPosition` until stack overflow (macOS 26.x). Hide lights, drop the
/// CSD `NSToolbar`, and zero titlebar height **before** `toggleFullScreen:`.
pub(crate) fn prep_native_fullscreen_exit(nswin: &NSWindow) {
    use objc2::runtime::AnyObject;
    use objc2::runtime::NSObjectProtocol;

    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(btn) = nswin.standardWindowButton(kind) {
            btn.setHidden(true);
        }
    }
    unsafe {
        let none: Option<&AnyObject> = None;
        let _: () = msg_send![nswin, setToolbar: none];
    }
    if nswin.respondsToSelector(objc2::sel!(setTitlebarHeight:)) {
        unsafe {
            let _: () = msg_send![nswin, setTitlebarHeight: 0.0f64];
        }
    }
    if let Some(cv) = nswin.contentView() {
        if let Some(frame) = unsafe { cv.superview() } {
            if frame.respondsToSelector(objc2::sel!(setCustomTitlebarHeight:)) {
                unsafe {
                    let _: () = msg_send![&*frame, setCustomTitlebarHeight: 0.0f64];
                }
            }
        }
    }
    crate::macos_fs_debug::log("prep native fullscreen exit (titlebar flattened)");
}

/// Hide or show the macOS traffic-light buttons on the NSWindow that hosts `widget`.
///
/// Uses [`NSWindow::standardWindowButton`] + `setHidden:`. We deliberately do **not**
/// touch GTK's `set_show_start_title_buttons` here: on macOS that path is one-way (once
/// disabled, GTK won't restore the AppKit buttons), and re-enabling it after a hide
/// fight breaks the very state we are trying to manage. Driving `setHidden:` directly is
/// reversible and survives GTK layout passes.
pub fn set_traffic_lights_visible<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    if crate::macos_fs_exit::exit_armed() {
        crate::macos_fs_debug::log("skip traffic lights (exit armed)");
        return;
    }
    let Some(win) = nswindow_for_widget(widget) else {
        return;
    };
    let hidden = !visible;
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(btn) = win.standardWindowButton(kind) {
            btn.setHidden(hidden);
        }
    }
    if visible {
        sync_traffic_lights_vertical(widget, widget.height());
    }
}
