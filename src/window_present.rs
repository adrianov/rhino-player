//! Present the main window on the display that contains the pointer.
//!
//! Portable: [`gtk::prelude::GtkWindowExt::present`] (compositor chooses placement; fine on Wayland).
//! macOS: set [`NSWindow`] frame on the screen under the mouse **before** the first `present` so the
//! first frame is not drawn on the primary display (gdk-macos would otherwise show, then jump on idle).
//! Screen placement runs **once** at startup; later activations only call [`GtkWindowExt::present`].

#[cfg(target_os = "macos")]
use glib::prelude::{Cast, ObjectExt};
use gtk::prelude::GtkWindowExt;
#[cfg(target_os = "macos")]
use gtk::prelude::{GtkApplicationExt, WidgetExt};

/// Install platform hooks (macOS: raise on re-activation without re-centering).
#[cfg(target_os = "macos")]
pub fn wire_activation_present(_app: &adw::Application) {
    #[cfg(target_os = "macos")]
    wire_macos_did_become_active(_app);
}

/// First present at startup; on macOS, center on the screen under the mouse when windowed.
pub fn present_on_activation_display(win: &adw::ApplicationWindow) {
    if win.is_fullscreen() || win.is_maximized() {
        win.present();
        return;
    }
    #[cfg(target_os = "macos")]
    present_on_mouse_screen_macos(win);
    #[cfg(not(target_os = "macos"))]
    win.present();
}

#[cfg(target_os = "macos")]
pub(crate) fn pick_application_window(app: &adw::Application) -> Option<adw::ApplicationWindow> {
    app.active_window()
        .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
        .or_else(|| {
            app.windows()
                .into_iter()
                .find_map(|w| w.downcast::<adw::ApplicationWindow>().ok())
        })
}

#[cfg(target_os = "macos")]
fn present_on_mouse_screen_macos(win: &adw::ApplicationWindow) {
    if !win.is_realized() {
        win.realize();
    }
    // Avoid one frame on the old monitor when the window is already visible there.
    if win.is_visible() {
        win.set_visible(false);
    }
    place_on_mouse_screen(win);
    win.present();
    // gdk-macos may still adjust the frame during present; re-apply synchronously (no idle).
    place_on_mouse_screen(win);
}

#[cfg(target_os = "macos")]
fn wire_macos_did_become_active(app: &adw::Application) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplicationDidBecomeActiveNotification;
    use objc2_foundation::NSNotificationCenter;

    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let app_weak = app.downgrade();
    let block = RcBlock::new(move |_notif| {
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        let Some(win) = pick_application_window(&app) else {
            return;
        };
        // Startup already placed the frame; re-centering on every click/active broke windowed UX.
        win.present();
    });
    let center = NSNotificationCenter::defaultCenter();
    let _observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidBecomeActiveNotification),
            None,
            None,
            &block,
        )
    };
    std::mem::forget(_observer);
}

#[cfg(target_os = "macos")]
fn place_on_mouse_screen(win: &adw::ApplicationWindow) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindowAnimationBehavior;

    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(nswin) = crate::macos_window::nswindow_for_widget(win) else {
        return;
    };
    nswin.setAnimationBehavior(NSWindowAnimationBehavior::None);
    let Some(screen) = screen_under_mouse_or_main() else {
        return;
    };
    let vis = screen.visibleFrame();
    let mut frame = nswin.frame();
    if frame.size.width < 64.0 {
        frame.size.width = f64::from(win.default_width().max(320));
    }
    if frame.size.height < 64.0 {
        frame.size.height = f64::from(win.default_height().max(200));
    }
    frame.origin.x = vis.origin.x + (vis.size.width - frame.size.width) / 2.0;
    frame.origin.y = vis.origin.y + (vis.size.height - frame.size.height) / 2.0;
    nswin.setFrame_display(frame, true);
}

#[cfg(target_os = "macos")]
fn screen_under_mouse_or_main() -> Option<objc2::rc::Retained<objc2_app_kit::NSScreen>> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};
    use objc2_foundation::NSRect;

    let mtm = MainThreadMarker::new()?;
    let loc = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    if let Some(screen) = screens.iter().find(|s| {
        let f: NSRect = s.frame();
        loc.x >= f.origin.x
            && loc.x < f.origin.x + f.size.width
            && loc.y >= f.origin.y
            && loc.y < f.origin.y + f.size.height
    }) {
        return Some(screen.clone());
    }
    NSScreen::mainScreen(mtm)
}
