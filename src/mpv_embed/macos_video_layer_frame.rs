//! Per-frame mirroring from a GTK widget's allocation onto a [`RhinoMpvGlLayer`]'s
//! Cocoa frame, plus the GTK signal wiring that drives it. Pulled out of
//! `macos_video_attach.rs` so each module stays under the soft 300-line limit.

#![allow(deprecated)]

use glib::object::IsA;
use glib::SignalHandlerId;
use gtk::prelude::{Cast, WidgetExt};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::{CALayer, CATransaction};

use crate::macos_window::nswindow_for_widget;

use super::macos_video_displaylink::DriverStateHandle;
use super::macos_video_layer::RhinoMpvGlLayer;

mod resync_ticker;
mod resync_wiring;

pub(super) type OverlayCell = std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>>;

fn translate_to_window<W: IsA<gtk::Widget>>(widget: &W, win: &gtk::Window) -> Option<(f64, f64)> {
    widget
        .compute_point(win, &gtk::graphene::Point::new(0.0, 0.0))
        .map(|p| (p.x() as f64, p.y() as f64))
}

/// NSWindow contentView height in points — read directly from AppKit so the layer's
/// Y-flip matches gdk-macos's compositing without a half-point drift around the chrome.
fn nswindow_content_height_for<W: IsA<gtk::Widget>>(sizer: &W) -> Option<f64> {
    let win = nswindow_for_widget(sizer)?;
    unsafe {
        let cv: *mut NSView = msg_send![&*win, contentView];
        if cv.is_null() {
            return None;
        }
        let frame: NSRect = msg_send![cv, frame];
        Some(frame.size.height)
    }
}

/// Whether the video layer should be visible: sizer visible+mapped, no overlay shown.
fn target_visible<W: IsA<gtk::Widget>>(sizer: &W, overlay: Option<&gtk::Widget>) -> bool {
    let overlay_shown = overlay.is_some_and(|w| w.is_visible());
    sizer.is_visible() && sizer.is_mapped() && !overlay_shown
}

/// Frame (in window coordinates) + bounds for the layer at sizer position (x, y).
fn layer_frames<W: IsA<gtk::Widget>>(
    sizer: &W,
    x: f64,
    y: f64,
    window: &gtk::Window,
) -> (NSRect, NSRect) {
    let w = (sizer.width() as f64).max(1.0);
    let h = (sizer.height() as f64).max(1.0);
    let win_h = nswindow_content_height_for(sizer).unwrap_or_else(|| window.height() as f64);
    let ns_y = win_h - y - h;
    (
        NSRect::new(NSPoint::new(x, ns_y), NSSize::new(w, h)),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h)),
    )
}

/// Target frame/bounds of the video layer in window coordinates, plus whether the
/// layer should be visible.
fn sync_geometry<W: IsA<gtk::Widget>>(
    sizer: &W,
    window: &gtk::Window,
    overlay: Option<&gtk::Widget>,
) -> Option<(NSRect, NSRect, bool)> {
    let (x, y) = translate_to_window(sizer, window)?;
    let (frame, bounds) = layer_frames(sizer, x, y, window);
    Some((frame, bounds, target_visible(sizer, overlay)))
}

/// Full GLArea allocation — chrome overlays the video via opaque gdk-macos widgets above this layer.
pub(super) fn sync_layer_frame_now<W: IsA<gtk::Widget>>(
    layer: &RhinoMpvGlLayer,
    sizer: &W,
    overlay: Option<&gtk::Widget>,
    repaint: Option<&DriverStateHandle>,
) {
    let Some(window) = sizer.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
        return;
    };
    let Some((frame, bounds, visible)) = sync_geometry(sizer, &window, overlay) else {
        return;
    };
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    unsafe {
        let _: () = msg_send![layer, setFrame: frame];
        let _: () = msg_send![layer, setBounds: bounds];
        let _: () = msg_send![layer, setHidden: !visible];
    }
    CATransaction::commit();
    if let Some(h) = repaint {
        h.mark_pending();
    }
}

/// After programmatic resize gdk-macos may stack a fresh GTK compositing layer above the video.
pub(super) fn pin_video_layer_below_gtk(layer: &RhinoMpvGlLayer) {
    unsafe {
        let superlayer: *mut CALayer = msg_send![layer, superlayer];
        if superlayer.is_null() {
            return;
        }
        let _: () = msg_send![superlayer, insertSublayer: layer, atIndex: 0u32];
    }
}

/// Mirror the `sizer` widget's allocation + visibility onto `layer` every frame. The
/// tick callback short-circuits no-op frames; `notify::root`, `notify::visible`,
/// `connect_map`, `notify::width` / `notify::height`, cover first attach + re-show +
/// live resize. **`repaint`**: after moving the layer, ask the display link for one draw so
/// mpv repaints into the new viewport (otherwise the last frame may stretch until the next
/// decoded frame).
pub(super) fn wire_sizer_resync(
    sizer_widget: &gtk::Widget,
    layer: Retained<RhinoMpvGlLayer>,
    overlay: OverlayCell,
    repaint: std::sync::Arc<DriverStateHandle>,
) -> SignalHandlerId {
    let id = resync_wiring::connect_root(sizer_widget, &layer, &overlay, &repaint);
    resync_wiring::connect_visible(sizer_widget, &layer, &overlay, &repaint);
    resync_wiring::connect_map(sizer_widget, &layer, &overlay, &repaint);
    resync_wiring::connect_size_notify(sizer_widget, &layer, &overlay, &repaint, "width");
    resync_wiring::connect_size_notify(sizer_widget, &layer, &overlay, &repaint, "height");
    resync_ticker::add_ticker(sizer_widget, layer, overlay, repaint);
    id
}
