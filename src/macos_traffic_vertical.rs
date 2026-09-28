// Native traffic lights for the compact ToolbarView top bar: visibility + frame sync.
//
// Y is always computed from fixed TOP_BAR_H (CSS compact header) — never from live
// reveal height. X uses AppKit’s slot with a minimum leading inset, cached once so
// compositing refresh cannot keep adjusting. Sync is a no-op while buttons are
// hidden so a mid-hide compositing pass cannot lock a bad X sample.

use objc2_foundation::NSPoint;

/// Matches `min-height` on `toolbarview.rp-toolbar headerbar.rpb-header`
/// (`theme/shell.css` / `macos_header_compact.css`).
const TOP_BAR_H: f64 = 34.0;

/// Minimum leading inset for the close button (titlebar coords). An earlier
/// fixed left-shift parked the lights on the window edge (~0–4 pt); keep AppKit’s
/// slot and only nudge up when a sample lands too close to the border.
const TRAFFIC_LIGHTS_MIN_X: f64 = 12.0;

thread_local! {
    static TRAFFIC_LIGHT_XS: RefCell<Option<(f64, f64, f64)>> = const { RefCell::new(None) };
}

fn leading_x(x: f64) -> f64 {
    x.max(TRAFFIC_LIGHTS_MIN_X)
}

fn stoplight_y(nswin: &NSWindow) -> Option<f64> {
    let close = nswin.standardWindowButton(NSWindowButton::CloseButton)?;
    let titlebar = unsafe { close.superview() }?;
    let title_h = titlebar.bounds().size.height;
    if title_h <= 0.0 {
        return None;
    }
    let h = close.frame().size.height;
    if h <= 0.0 {
        return None;
    }
    let band_h = TOP_BAR_H.min(title_h);
    let band_base = title_h - band_h;
    Some(band_base + (band_h - h) * 0.5)
}

fn remember_xs(nswin: &NSWindow) -> Option<(f64, f64, f64)> {
    let close = nswin.standardWindowButton(NSWindowButton::CloseButton)?;
    let mini = nswin.standardWindowButton(NSWindowButton::MiniaturizeButton)?;
    let zoom = nswin.standardWindowButton(NSWindowButton::ZoomButton)?;
    let close_x = close.frame().origin.x;
    let nudge = (TRAFFIC_LIGHTS_MIN_X - close_x).max(0.0);
    Some((
        leading_x(close_x),
        mini.frame().origin.x + nudge,
        zoom.frame().origin.x + nudge,
    ))
}

fn set_buttons_hidden(nswin: &NSWindow, hidden: bool) {
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(btn) = nswin.standardWindowButton(kind) {
            btn.setHidden(hidden);
        }
    }
}

fn apply_origins(nswin: &NSWindow, xs: (f64, f64, f64), y: f64) {
    for (kind, x) in [
        (NSWindowButton::CloseButton, xs.0),
        (NSWindowButton::MiniaturizeButton, xs.1),
        (NSWindowButton::ZoomButton, xs.2),
    ] {
        let Some(btn) = nswin.standardWindowButton(kind) else {
            continue;
        };
        btn.setFrameOrigin(NSPoint::new(x, y));
    }
}

fn clear_traffic_light_xs() {
    TRAFFIC_LIGHT_XS.with(|cell| *cell.borrow_mut() = None);
}

/// Hide stoplights and drop the X cache — used from [`prep_native_fullscreen_exit`].
pub(crate) fn flatten_traffic_lights(nswin: &NSWindow) {
    clear_traffic_light_xs();
    set_buttons_hidden(nswin, true);
}

/// Align stoplights to the fixed compact top bar. No-op while hidden.
pub fn sync_traffic_lights_vertical<W: IsA<gtk::Widget>>(anchor: &W) {
    let Some(nswin) = nswindow_for_widget(anchor) else {
        return;
    };
    let Some(close) = nswin.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    if close.isHidden() {
        return;
    }
    let Some(y) = stoplight_y(&nswin) else {
        return;
    };
    let xs = TRAFFIC_LIGHT_XS.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = remember_xs(&nswin);
        }
        *slot
    });
    let Some(xs) = xs else {
        return;
    };
    apply_origins(&nswin, xs, y);
}

/// Hide or show the macOS traffic-light buttons on the NSWindow that hosts `widget`.
///
/// Uses [`NSWindow::standardWindowButton`] + `setHidden:`. We deliberately do **not**
/// touch GTK's `set_show_start_title_buttons` here: on macOS that path is one-way (once
/// disabled, GTK won't restore the AppKit buttons). Driving `setHidden:` directly is
/// reversible and survives GTK layout passes.
pub fn set_traffic_lights_visible<W: IsA<gtk::Widget>>(widget: &W, visible: bool) {
    if crate::macos_fs_exit::exit_armed() {
        crate::macos_fs_debug::log("skip traffic lights (exit armed)");
        return;
    }
    let Some(win) = nswindow_for_widget(widget) else {
        return;
    };
    set_buttons_hidden(&win, !visible);
    if visible {
        // A stale exit-cycle NSToolbar indents the lights ~20px right; every chrome reveal
        // must present them at the standard slot.
        crate::macos_window::clear_stale_titlebar_toolbar(&win);
        sync_traffic_lights_vertical(widget);
    }
}
