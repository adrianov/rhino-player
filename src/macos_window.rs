//! macOS-only helpers around the native [`NSWindow`] hosting a [`gtk::Window`].
//!
//! Resolves the underlying NSWindow via `gdk4_macos::MacosSurface::native()` (GTK 4.8+)
//! and exposes one operation today: hide / show the standard "traffic-light" buttons
//! together with our chrome auto-hide.

use gdk4_macos::MacosSurface;
use gdk4_macos::prelude::Cast;
use glib::object::IsA;
use gtk::prelude::{NativeExt, WidgetExt};
use objc2::rc::Retained;
use objc2_app_kit::{NSWindow, NSWindowButton};

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
