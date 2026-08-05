//! macOS-only helpers around the native [`NSWindow`] hosting a [`gtk::Window`].
//!
//! Resolves the underlying NSWindow via `gdk4_macos::MacosSurface::native()` (GTK 4.8+)
//! and exposes helpers used by the GTK shell: hide / show traffic lights and layer invalidation.
//!
//! Fullscreen **exit** (`chrome_macos_unfullscreen_defer`): arm guard, settle, libdispatch
//! hop, then [`toggleFullScreen:`] — not [`GtkWindowExt::set_fullscreened`](false). Toolbar
//! reveal waits for `fullscreened_notify` (never while the native fullscreen mask is set).

use gdk4_macos::prelude::Cast;
use gdk4_macos::MacosSurface;
use glib::object::IsA;
use gtk::prelude::{GtkWindowExt, NativeExt, WidgetExt};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{NSCursor, NSView, NSWindow, NSWindowButton, NSWindowStyleMask};
use std::cell::{Cell, RefCell};

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
        let Some(content_view) = Retained::retain(cv) else {
            return;
        };
        let _: () = msg_send![&*content_view, setNeedsDisplay: true];
        let _: () = msg_send![&*content_view, displayIfNeeded];
    }
}

#[cfg(target_os = "macos")]
include!("macos_window_gdk_layout.rs");

#[cfg(target_os = "macos")]
include!("macos_traffic_vertical.rs");

#[cfg(target_os = "macos")]
include!("macos_window_fs.rs");

#[cfg(target_os = "macos")]
include!("macos_window_cursor.rs");

#[cfg(not(target_os = "macos"))]
pub fn resize_window_frame(_win: &adw::ApplicationWindow, _width: i32, _height: i32) {}

#[cfg(not(target_os = "macos"))]
pub fn request_gdk_surface_layout<W: IsA<gtk::Widget>>(_widget: &W) {}

#[cfg(not(target_os = "macos"))]
pub fn refresh_gdk_shell_compositing(
    _win: &adw::ApplicationWindow,
    _gl: &gtk::GLArea,
    _header: &adw::HeaderBar,
    _root: &adw::ToolbarView,
    _bottom_shell: &gtk::Box,
    _bottom: &gtk::Box,
) {
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn schedule_shell_layout_after_gtk_resize(_target_w: i32, _target_h: i32) {}
