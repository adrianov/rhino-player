//! macOS-only helpers around the native [`NSWindow`] hosting a [`gtk::Window`].
//!
//! Resolves the underlying NSWindow via `gdk4_macos::MacosSurface::native()` (GTK 4.8+)
//! and exposes helpers used by the GTK shell: hide / show traffic lights and layer invalidation.
//!
//! Fullscreen **exit** (`chrome_macos_unfullscreen_defer`): reveal toolbar bars, settle,
//! then native [`toggleFullScreen:`] while [`crate::macos_fs_exit`] is armed — not
//! [`GtkWindowExt::set_fullscreened`](false).

use gdk4_macos::prelude::Cast;
use gdk4_macos::MacosSurface;
use glib::object::IsA;
use gtk::prelude::{GtkWindowExt, NativeExt, WidgetExt};
#[cfg(target_os = "macos")]
use glib::prelude::ObjectType;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{NSCursor, NSView, NSWindow, NSWindowButton, NSWindowStyleMask};
use std::cell::Cell;
#[cfg(target_os = "macos")]
use std::cell::RefCell;
#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::rc::Rc;

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct WinFsExitState {
    bar_show: Rc<Cell<bool>>,
    toolbar: adw::ToolbarView,
}

#[cfg(target_os = "macos")]
thread_local! {
    static WIN_FS_EXIT: RefCell<HashMap<isize, WinFsExitState>> = RefCell::new(HashMap::new());
}

/// Per-window fullscreen-exit state (`bar_show` + root [`ToolbarView`]); dropped on window destroy.
#[cfg(target_os = "macos")]
pub(crate) fn register_win_bar_show(
    win: &adw::ApplicationWindow,
    bar_show: Rc<Cell<bool>>,
    toolbar: adw::ToolbarView,
) {
    let key = win.as_ptr() as isize;
    WIN_FS_EXIT.with(|m| {
        m.borrow_mut().insert(
            key,
            WinFsExitState {
                bar_show,
                toolbar,
            },
        );
    });
    win.connect_destroy(move |_| {
        WIN_FS_EXIT.with(|m| m.borrow_mut().remove(&key));
    });
}

#[cfg(target_os = "macos")]
fn fs_exit_state_for_win(win: &adw::ApplicationWindow) -> Option<WinFsExitState> {
    let key = win.as_ptr() as isize;
    WIN_FS_EXIT.with(|m| m.borrow().get(&key).cloned())
}

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

/// Before AppKit leaves fullscreen: reveal [`ToolbarView`] bars only.
///
/// Enter fullscreen sets `bar_show` false and hides bars; touching traffic-light /
/// `setShowStandardWindowButtons` in the same turn as `toggleFullScreen:` retriggers
/// `_NSThemeZoomWidgetCell` geometry and can recurse titlebar layout. Native buttons
/// stay visible while fullscreen (`chrome_header_csd_controls` fullscreen branch).
pub(crate) fn prepare_fullscreen_exit(win: &adw::ApplicationWindow) {
    if let Some(st) = fs_exit_state_for_win(win) {
        st.bar_show.set(true);
        st.toolbar.set_reveal_top_bars(true);
        st.toolbar.set_reveal_bottom_bars(true);
    }
    crate::macos_fs_debug::log("prepare exit: reveal toolbar bars only");
}

/// Hide or show the macOS traffic-light buttons on the NSWindow that hosts `widget`.
///
/// Uses [`NSWindow::standardWindowButton`] + `setHidden:`. We deliberately do **not**
/// touch GTK's `set_show_start_title_buttons` here: on macOS that path is one-way (once
/// disabled, GTK won't restore the AppKit buttons), and re-enabling it after a hide
/// fight breaks the very state we are trying to manage. Driving `setHidden:` directly is
/// reversible and survives GTK layout passes.
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

/// GDK-style guarded `toggleFullScreen:` (same path as `_gdk_macos_toplevel_surface_unfullscreen`).
pub(crate) fn native_toggle_fullscreen_exit(win: &adw::ApplicationWindow) -> bool {
    let gtk = win.upcast_ref::<gtk::Widget>();
    let Some(nswin) = nswindow_for_widget(gtk) else {
        return false;
    };
    if gdk_macos_in_fullscreen_transition(gtk) || !ns_window_is_native_fullscreen(&nswin) {
        return false;
    }
    unsafe {
        let _: () = msg_send![&*nswin, toggleFullScreen: &*nswin];
    }
    true
}

pub fn set_traffic_lights_visible<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    if crate::macos_fs_exit::exit_armed() && !visible {
        crate::macos_fs_debug::log("skip traffic lights hide (exit armed)");
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
