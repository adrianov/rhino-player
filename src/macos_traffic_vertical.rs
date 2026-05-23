// Vertically center native traffic lights in the compact ToolbarView top bar.

use objc2_foundation::NSPoint;

/// AppKit’s default stoplight X sits too far right against our compact header chrome.
const TRAFFIC_LIGHTS_SHIFT_LEFT: f64 = 8.0;

#[derive(Clone, Copy)]
struct BtnFrame {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy)]
struct TrafficLightFrames {
    close: BtnFrame,
    mini: BtnFrame,
    zoom: BtnFrame,
}

thread_local! {
    static TRAFFIC_LIGHT_FRAMES: RefCell<Option<TrafficLightFrames>> = const { RefCell::new(None) };
}

fn shifted_x(x: f64) -> f64 {
    if x >= TRAFFIC_LIGHTS_SHIFT_LEFT {
        (x - TRAFFIC_LIGHTS_SHIFT_LEFT).max(0.0)
    } else {
        x
    }
}

fn stoplight_y(nswin: &NSWindow, bar_h: i32) -> Option<f64> {
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
    let gtk_h = f64::from(bar_h);
    let band_h = gtk_h.min(title_h);
    let band_base = title_h - band_h;
    Some(band_base + (band_h - h) * 0.5)
}

fn remember_first_frames(nswin: &NSWindow, bar_h: i32) -> Option<TrafficLightFrames> {
    let y = stoplight_y(nswin, bar_h)?;
    let close = nswin.standardWindowButton(NSWindowButton::CloseButton)?;
    let mini = nswin.standardWindowButton(NSWindowButton::MiniaturizeButton)?;
    let zoom = nswin.standardWindowButton(NSWindowButton::ZoomButton)?;
    Some(TrafficLightFrames {
        close: BtnFrame {
            x: shifted_x(close.frame().origin.x),
            y,
        },
        mini: BtnFrame {
            x: shifted_x(mini.frame().origin.x),
            y,
        },
        zoom: BtnFrame {
            x: shifted_x(zoom.frame().origin.x),
            y,
        },
    })
}

fn apply_traffic_light_frames(nswin: &NSWindow, frames: TrafficLightFrames) {
    for (kind, pt) in [
        (NSWindowButton::CloseButton, frames.close),
        (NSWindowButton::MiniaturizeButton, frames.mini),
        (NSWindowButton::ZoomButton, frames.zoom),
    ] {
        let Some(btn) = nswin.standardWindowButton(kind) else {
            continue;
        };
        btn.setFrameOrigin(NSPoint::new(pt.x, pt.y));
    }
}

/// First draw remembers exact stoplight origins; every later call re-applies the same frames.
pub fn sync_traffic_lights_vertical<W: IsA<gtk::Widget>>(anchor: &W, bar_h: i32) {
    let Some(nswin) = nswindow_for_widget(anchor) else {
        return;
    };
    TRAFFIC_LIGHT_FRAMES.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            if bar_h < 20 {
                return;
            }
            let Some(frames) = remember_first_frames(&nswin, bar_h) else {
                return;
            };
            *slot = Some(frames);
        }
        if let Some(frames) = *slot {
            apply_traffic_light_frames(&nswin, frames);
        }
    });
}
