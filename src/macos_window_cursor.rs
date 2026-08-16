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
    let gw = gtk_win.width() as f64;
    let gh = gtk_win.height() as f64;
    if gw <= 1.0 || gh <= 1.0 {
        return None;
    }
    let gtk_x = p.x;
    let gtk_y = gh - p.y;
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
    let loc = NSEvent::mouseLocation();
    let front = NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(loc, 0, mtm);
    front == nswin.windowNumber()
}

thread_local! {
    /// Paired hide/show so the per-display CoreGraphics hide count stays balanced.
    static CURSOR_HIDDEN: Cell<bool> = const { Cell::new(false) };
    /// Display id passed to [`CGDisplayHideCursor`]; show must use the same id.
    static HIDDEN_DISPLAY: Cell<Option<u32>> = const { Cell::new(None) };
}

#[repr(C)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
    fn CGGetDisplaysWithPoint(
        point: CgPoint,
        max_displays: u32,
        displays: *mut u32,
        matching_display_count: *mut u32,
    ) -> i32;
}

fn display_at_pointer() -> Option<u32> {
    use objc2_app_kit::NSEvent;
    let loc = NSEvent::mouseLocation();
    let mut id = 0u32;
    let mut n = 0u32;
    let err = unsafe {
        CGGetDisplaysWithPoint(CgPoint { x: loc.x, y: loc.y }, 1, &mut id, &mut n)
    };
    (err == 0 && n > 0).then_some(id)
}

fn apply_cg_cursor(hidden: bool, display: u32) -> bool {
    if CURSOR_HIDDEN.get() == hidden {
        return hidden;
    }
    let err = unsafe {
        if hidden {
            CGDisplayHideCursor(display)
        } else {
            CGDisplayShowCursor(display)
        }
    };
    if err != 0 {
        eprintln!(
            "[rhino] cursor: CoreGraphics {} display={display} failed err={err}",
            if hidden { "hide" } else { "show" }
        );
        return CURSOR_HIDDEN.get();
    }
    CURSOR_HIDDEN.set(hidden);
    HIDDEN_DISPLAY.set(hidden.then_some(display));
    hidden
}

/// Hide the system cursor on the display where `win` is under the pointer.
///
/// No-op (and shows if we had hidden) when the pointer is on another screen. Returns whether
/// the cursor is hidden.
pub fn hide_system_cursor(win: &adw::ApplicationWindow) -> bool {
    if !window_frontmost_at_pointer(win) {
        show_system_cursor();
        return false;
    }
    let Some(display) = display_at_pointer() else {
        show_system_cursor();
        return false;
    };
    apply_cg_cursor(true, display)
}

/// Show the system cursor on the display that was hidden (if any).
pub fn show_system_cursor() {
    let Some(display) = HIDDEN_DISPLAY.get() else {
        return;
    };
    let _ = apply_cg_cursor(false, display);
}
