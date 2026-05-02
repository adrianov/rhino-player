//! macOS-only helpers around the native [`NSWindow`] hosting a [`gtk::Window`].
//!
//! Resolves the underlying NSWindow via `gdk4_macos::MacosSurface::native()` (GTK 4.8+)
//! and exposes one operation today: hide / show the standard "traffic-light" buttons
//! together with our chrome auto-hide.

use gdk4_macos::MacosSurface;
use gdk4_macos::prelude::Cast;
use glib::object::IsA;
use gtk::prelude::{NativeExt, WidgetExt};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton};

/// Resolve the underlying [`NSWindow`] for a realized GTK widget on macOS.
///
/// Returns `None` before the GtkWindow is realized (no surface yet) or on non-macOS
/// surfaces. Shared with [`crate::mpv_embed::macos_video_attach`] so the gdk-macos
/// → AppKit conversion lives in exactly one place.
pub fn nswindow_for_widget<W: IsA<gtk::Widget>>(w: &W) -> Option<Retained<NSWindow>> {
    let surface = w.native()?.surface()?;
    let macos = surface.downcast::<MacosSurface>().ok()?;
    let ptr = macos.native() as *mut NSWindow;
    if ptr.is_null() {
        return None;
    }
    unsafe { Retained::retain(ptr) }
}

/// Invalidate the contentView's layer tree and force an immediate redraw.
///
/// AppKit snapshots the contentView's layer tree when the window leaves the active
/// Space (focus moves to a different display or desktop) and replays the snapshot on
/// the way back as a cross-fade. With our hybrid setup — native `CAOpenGLLayer` at
/// index 0 of `contentView.layer.sublayers`, gdk-macos's GTK rendering above it — the
/// cross-fade can leave gdk-macos's chrome sublayer with stale, stretched contents
/// that show up as a horizontal band of header chrome through the middle of the
/// video. `setNeedsDisplay:YES` + `displayIfNeeded` on the contentView drops the
/// cached backing store and asks gdk-macos for a fresh draw on the spot.
///
/// No-op before the surface is realized.
pub fn invalidate_window_layers<W: IsA<gtk::Widget>>(widget: &W) {
    let Some(win) = nswindow_for_widget(widget) else {
        return;
    };
    unsafe {
        let cv: *mut NSView = msg_send![&*win, contentView];
        let Some(content_view) = Retained::retain(cv) else { return };
        let _: () = msg_send![&*content_view, setNeedsDisplay: true];
        let _: () = msg_send![&*content_view, displayIfNeeded];
    }
}

/// Hide or show the macOS traffic-light buttons on the NSWindow that hosts `widget`.
///
/// Uses [`NSWindow::standardWindowButton`] + `setHidden:`. We deliberately do **not**
/// touch GTK's `set_show_start_title_buttons` here: on macOS that path is one-way (once
/// disabled, GTK won't restore the AppKit buttons), and re-enabling it after a hide
/// fight breaks the very state we are trying to manage. Driving `setHidden:` directly is
/// reversible and survives GTK layout passes.
pub fn set_traffic_lights_visible<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
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
}
