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
    let gtk_x = p.x;
    let gtk_y = gh - p.y;
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
