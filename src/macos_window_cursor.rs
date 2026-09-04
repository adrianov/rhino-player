/// Coordinates for [`gtk::Widget::pick`] on `gtk_win`, from [`NSWindow::mouseLocationOutsideOfEventStream`].
///
/// Still correct when the window is **not key** — GDK [`DeviceExt::surface_at_position`] often omits our
/// surface in that case. Gtk uses a top-left origin; NSWindow base uses bottom-left, so **Y is flipped**.
/// Returns [`None`] when another window is frontmost at the pointer (geometry inside our frame is not enough).
pub fn mouse_point_for_gtk_pick(gtk_win: &adw::ApplicationWindow) -> Option<(f64, f64)> {
    if !window_frontmost_at_pointer(gtk_win) {
        return None;
    }
    let nswin = nswindow_for_widget(gtk_win.upcast_ref::<gtk::Widget>())?;
    let p = nswin.mouseLocationOutsideOfEventStream();
    gtk_pick_point(gtk_win, p.x, p.y)
}

/// Flip NSWindow bottom-left Y into GTK top-left and reject points outside the window.
fn gtk_pick_point(win: &adw::ApplicationWindow, ns_x: f64, ns_y: f64) -> Option<(f64, f64)> {
    let gw = win.width() as f64;
    let gh = win.height() as f64;
    if gw <= 1.0 || gh <= 1.0 {
        return None;
    }
    let gtk_x = ns_x;
    let gtk_y = gh - ns_y;
    if gtk_x < 0.0 || gtk_y < 0.0 || gtk_x > gw || gtk_y > gh {
        return None;
    }
    Some((gtk_x, gtk_y))
}

pub(crate) fn window_frontmost_at_pointer(gtk_win: &adw::ApplicationWindow) -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSEvent;

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(nswin) = nswindow_for_widget(gtk_win.upcast_ref::<gtk::Widget>()) else {
        return false;
    };
    NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(NSEvent::mouseLocation(), 0, mtm)
        == nswin.windowNumber()
}

fn pointer_in_window_frame(gtk_win: &adw::ApplicationWindow) -> bool {
    use objc2_app_kit::NSEvent;

    let Some(nswin) = nswindow_for_widget(gtk_win.upcast_ref::<gtk::Widget>()) else {
        return false;
    };
    let loc = NSEvent::mouseLocation();
    let f = nswin.frame();
    loc.x >= f.origin.x
        && loc.y >= f.origin.y
        && loc.x <= f.origin.x + f.size.width
        && loc.y <= f.origin.y + f.size.height
}

thread_local! {
    static THEATER_HIDDEN: Cell<bool> = const { Cell::new(false) };
}

// Display id is ignored by CoreGraphics; hide/show is process-wide and must stay paired.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
}

fn cg_set(hidden: bool) {
    let err = unsafe {
        if hidden {
            CGDisplayHideCursor(0)
        } else {
            CGDisplayShowCursor(0)
        }
    };
    if err != 0 {
        eprintln!(
            "[rhino] cursor: CoreGraphics {} failed err={err}",
            if hidden { "hide" } else { "show" }
        );
    }
}

/// Hide while the pointer is inside the viewer. Does not show on skip (leave paths show).
pub fn hide_system_cursor(win: &adw::ApplicationWindow) -> bool {
    if !pointer_in_window_frame(win) {
        return false;
    }
    if !THEATER_HIDDEN.get() {
        cg_set(true);
        THEATER_HIDDEN.set(true);
    }
    true
}

pub fn show_system_cursor() {
    if THEATER_HIDDEN.replace(false) {
        cg_set(false);
    }
}

struct CoverIvars;

use objc2::{define_class, AllocAnyThread, MainThreadOnly};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSCursor, NSImage};
use objc2_foundation::{NSObjectProtocol, NSRect, NSSize};

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "RhinoBlackoutView"]
    #[ivars = CoverIvars]
    struct RhinoBlackoutView;

    unsafe impl NSObjectProtocol for RhinoBlackoutView {}

    impl RhinoBlackoutView {
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            self.addCursorRect_cursor(self.bounds(), &blank_ns_cursor());
        }
    }
);

fn blank_ns_cursor() -> Retained<NSCursor> {
    NSCursor::initWithImage_hotSpot(
        NSCursor::alloc(),
        &NSImage::initWithSize(NSImage::alloc(), NSSize::new(1.0, 1.0)),
        NSPoint { x: 0.0, y: 0.0 },
    )
}

/// Cover windows ignore-mouse would show the desktop pointer; this view owns a blank cursor instead.
pub fn attach_blank_cursor_content(win: &NSWindow) {
    use objc2::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[rhino] cursor: blank cover view needs the main thread");
        return;
    };
    let this = RhinoBlackoutView::alloc(mtm).set_ivars(CoverIvars);
    let view: Retained<RhinoBlackoutView> = unsafe {
        msg_send![super(this), initWithFrame: NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: win.frame().size,
        }]
    };
    view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    let view = view.into_super();
    win.setContentView(Some(&view));
    win.invalidateCursorRectsForView(&view);
}
