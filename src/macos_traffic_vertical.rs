// Vertically center native traffic lights in the compact ToolbarView top bar.

use objc2_foundation::NSPoint;

/// AppKit’s default stoplight X sits too far right against our compact header chrome.
const TRAFFIC_LIGHTS_SHIFT_LEFT: f64 = 8.0;

/// Align stoplights with the gdk top chrome row (`bar_h` from [`gtk::Widget::height`]).
pub fn sync_traffic_lights_vertical<W: IsA<gtk::Widget>>(anchor: &W, bar_h: i32) {
    let Some(nswin) = nswindow_for_widget(anchor) else {
        return;
    };
    if bar_h < 20 {
        return;
    }
    let gtk_h = f64::from(bar_h);
    let Some(close) = nswin.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let titlebar = unsafe { close.superview() };
    let Some(titlebar) = titlebar else {
        return;
    };
    let title_h = titlebar.bounds().size.height;
    if title_h <= 0.0 {
        return;
    }
    let band_h = gtk_h.min(title_h);
    let band_base = title_h - band_h;
    let close_x = close.frame().origin.x;
    // Shift left once; repeated compositing refresh must not keep subtracting (yellow drifts toward red).
    let shift_x = close_x >= TRAFFIC_LIGHTS_SHIFT_LEFT;
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        let Some(btn) = nswin.standardWindowButton(kind) else {
            continue;
        };
        let h = btn.frame().size.height;
        if h <= 0.0 {
            continue;
        }
        let y = band_base + (band_h - h) * 0.5;
        let x = if shift_x {
            (btn.frame().origin.x - TRAFFIC_LIGHTS_SHIFT_LEFT).max(0.0)
        } else {
            btn.frame().origin.x
        };
        btn.setFrameOrigin(NSPoint::new(x, y));
    }
}
