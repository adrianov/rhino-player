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
use objc2_app_kit::{NSCursor, NSView, NSWindow, NSWindowButton};
use std::cell::Cell;

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

/// Coordinates for [`gtk::Widget::pick`] on `gtk_win`, from [`NSWindow::mouseLocationOutsideOfEventStream`].
///
/// Still correct when the window is **not key** — GDK [`DeviceExt::surface_at_position`] often omits our
/// surface in that case. Gtk uses a top-left origin; NSWindow base uses bottom-left, so **Y is flipped**.
pub fn mouse_point_for_gtk_pick(gtk_win: &adw::ApplicationWindow) -> Option<(f64, f64)> {
    let nswin = nswindow_for_widget(gtk_win.upcast_ref::<gtk::Widget>())?;
    let p = nswin.mouseLocationOutsideOfEventStream();
    let gw = gtk_win.width() as f64;
    let gh = gtk_win.height() as f64;
    if gw <= 1.0 || gh <= 1.0 {
        return None;
    }
    let gtk_x = p.x as f64;
    let gtk_y = gh - (p.y as f64);
    if gtk_x < 0.0 || gtk_y < 0.0 || gtk_x > gw || gtk_y > gh {
        return None;
    }
    Some((gtk_x, gtk_y))
}

thread_local! {
    /// Tracks whether we have called [`NSCursor::hide`] so calls stay balanced with
    /// [`NSCursor::unhide`]. `NSCursor` maintains a **global** hide count — unbalanced
    /// calls leave every other app's cursor invisible.
    static CURSOR_HIDDEN: Cell<bool> = const { Cell::new(false) };
}

/// Hide / show the **system** cursor via [`NSCursor::hide`] / [`NSCursor::unhide`].
///
/// Use this on macOS when GTK cursor rects are not honored — e.g. the window is **not** the key
/// window ([`gtk::Widget::set_cursor_from_name`] goes through AppKit paths that may only apply to
/// the key window), or when a **transparent** [`gtk::GLArea`] sits over a native video layer so the
/// pointer is still composited as the arrow. `NSCursor` hide/unhide work regardless, at the cost
/// of being process-wide — the matching `unhide` call **must** run before the pointer leaves our
/// window (otherwise other windows inherit the hidden cursor).
pub fn set_system_cursor_hidden(hidden: bool) {
    CURSOR_HIDDEN.with(|flag| {
        if flag.get() == hidden {
            return;
        }
        flag.set(hidden);
        if hidden {
            NSCursor::hide();
        } else {
            NSCursor::unhide();
        }
    });
}
