fn show_chrome_pointer(win: &adw::ApplicationWindow, gl: &gtk::GLArea) {
    #[cfg(target_os = "macos")]
    crate::macos_window::set_system_cursor_hidden(false);
    win.set_cursor_from_name(None);
    show_pointer(gl);
}

/// Hide the pointer after idle motion on the video [`GLArea`] (theater-style).
///
/// **Windows:** GTK cursor name `"none"` is enough. **macOS:** the native video layer often sits
/// under a transparent [`GLArea`], so AppKit may ignore GTK cursor rects; use
/// [`crate::macos_window::set_system_cursor_hidden`] the same way as after chrome auto-hide.
///
/// Returns whether the cursor was hidden (false when there is no real media to treat as theater).
fn apply_theater_cursor_hide(
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) -> bool {
    if !chrome_should_hide_cursor_for_media(player) {
        return false;
    }
    gl.add_css_class("rp-cursor-hidden");
    win.set_cursor_from_name(Some("none"));
    gl.set_cursor_from_name(Some("none"));
    #[cfg(target_os = "macos")]
    crate::macos_window::set_system_cursor_hidden(true);
    true
}

fn pointer_in_window_client(win: &adw::ApplicationWindow, x: f64, y: f64) -> bool {
    let w = win.width() as f64;
    let h = win.height() as f64;
    w > 0.0
        && h > 0.0
        && x >= 0.0
        && y >= 0.0
        && x <= w
        && y <= h
}

/// True when mpv has real media (not the idle / welcome state). Used before theater cursor hide.
fn chrome_should_hide_cursor_for_media(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    let Ok(b) = player.try_borrow() else {
        return false;
    };
    let Some(bundle) = b.as_ref() else {
        return false;
    };
    if bundle.mpv.get_property::<bool>("idle-active").unwrap_or(true) {
        return false;
    }
    match bundle.mpv.get_property::<String>("path") {
        Ok(p) => {
            let t = p.trim();
            !t.is_empty() && t != "null://"
        }
        Err(_) => false,
    }
}

fn pointer_pick_xy_for_win(win: &adw::ApplicationWindow) -> Option<(f64, f64)> {
    use gtk::gdk::prelude::{DeviceExt, DisplayExt, SeatExt};
    use gtk::prelude::NativeExt;

    let gdk_xy = (|| {
        let disp = gtk::gdk::Display::default()?;
        let seat = disp.default_seat()?;
        let dev = seat.pointer()?;
        let (surf, x, y) = dev.surface_at_position();
        let win_surf = win.surface()?;
        let surf = surf?;
        if surf != win_surf {
            return None;
        }
        Some((x, y))
    })();

    #[cfg(target_os = "macos")]
    {
        if let Some(xy) = gdk_xy {
            return Some(xy);
        }
        crate::macos_window::mouse_point_for_gtk_pick(win)
    }
    #[cfg(not(target_os = "macos"))]
    gdk_xy
}

/// After chrome hides: hide pointer only with **real** media, pointer **strictly inside** the window,
/// and [`gtk::Widget::pick`] over the [`GLArea`]. macOS uses [`NSCursor::hide`] only in that case;
/// otherwise we ensure any prior system hide is cleared.
fn hide_cursor_after_bars_hide(
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::Box,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    use glib::prelude::Cast;

    #[cfg(target_os = "macos")]
    crate::macos_window::set_system_cursor_hidden(false);

    if recent.is_visible() || !chrome_should_hide_cursor_for_media(player) {
        return;
    }

    let Some((pick_x, pick_y)) = pointer_pick_xy_for_win(win) else {
        return;
    };

    if !pointer_in_window_client(win, pick_x, pick_y) {
        return;
    }

    let gl_w: gtk::Widget = gl.clone().upcast();
    let over_gl = win
        .pick(pick_x, pick_y, gtk::PickFlags::DEFAULT)
        .is_some_and(|p| p == gl_w);
    if !over_gl {
        return;
    }

    apply_theater_cursor_hide(win, gl, player);
}
