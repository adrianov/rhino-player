//! Per-frame mirroring from a GTK widget's allocation onto a [`RhinoMpvGlLayer`]'s
//! Cocoa frame, plus the GTK signal wiring that drives it. Pulled out of
//! `macos_video_attach.rs` so each module stays under the soft 300-line limit.

#![allow(deprecated)]

use glib::object::{IsA, ObjectExt};
use glib::SignalHandlerId;
use gtk::prelude::{Cast, WidgetExt, WidgetExtManual};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::CATransaction;

use crate::macos_window::nswindow_for_widget;

use super::macos_video_layer::RhinoMpvGlLayer;

type OverlayCell = std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>>;

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

pub(super) fn sync_layer_frame_now<W: IsA<gtk::Widget>>(
    layer: &RhinoMpvGlLayer,
    sizer: &W,
    overlay: Option<&gtk::Widget>,
) {
    let Some(window) = sizer.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
        return;
    };
    let Some((x, y)) = translate_to_window(sizer, &window) else {
        return;
    };
    let overlay_shown = overlay.is_some_and(|w| w.is_visible());
    let visible = sizer.is_visible() && sizer.is_mapped() && !overlay_shown;
    let w = (sizer.width() as f64).max(1.0);
    let h = (sizer.height() as f64).max(1.0);
    let win_h = nswindow_content_height_for(sizer).unwrap_or_else(|| window.height() as f64);
    let ns_y = win_h - y - h;
    let frame = NSRect::new(NSPoint::new(x, ns_y), NSSize::new(w, h));
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h));
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    unsafe {
        let _: () = msg_send![layer, setFrame: frame];
        let _: () = msg_send![layer, setBounds: bounds];
        let _: () = msg_send![layer, setHidden: !visible];
    }
    CATransaction::commit();
}

/// Mirror the `sizer` widget's allocation + visibility onto `layer` every frame. The
/// tick callback short-circuits no-op frames; `notify::root`, `notify::visible`,
/// `connect_map` cover first attach + re-show.
pub(super) fn wire_sizer_resync(
    sizer_widget: &gtk::Widget,
    layer: Retained<RhinoMpvGlLayer>,
    overlay: OverlayCell,
) -> SignalHandlerId {
    let l_root = layer.clone();
    let s_root = sizer_widget.clone();
    let ov_root = overlay.clone();
    let id = sizer_widget.connect_local("notify::root", false, move |_| {
        let ov = ov_root.borrow().clone();
        sync_layer_frame_now(&l_root, &s_root, ov.as_ref());
        None
    });

    let l_vis = layer.clone();
    let ov_vis = overlay.clone();
    sizer_widget.connect_local("notify::visible", false, move |args| {
        if let Ok(w) = args[0].get::<gtk::Widget>() {
            let ov = ov_vis.borrow().clone();
            sync_layer_frame_now(&l_vis, &w, ov.as_ref());
        }
        None
    });

    let l_map = layer.clone();
    let ov_map = overlay.clone();
    sizer_widget.connect_map(move |w| {
        let ov = ov_map.borrow().clone();
        sync_layer_frame_now(&l_map, w, ov.as_ref());
    });

    let l_tick = layer;
    // Tick debounce key uses AppKit contentView height (`nswindow_content_height_for`), matching
    // [`sync_layer_frame_now`] — GTK `Window::height` alone can lag or diverge during maximize.
    let last = std::cell::Cell::new((0i32, 0i32, i64::MIN, false, false));
    sizer_widget.add_tick_callback(move |w, _| {
        let win_h_fallback = w
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
            .map(|win| win.height())
            .unwrap_or(0);
        let snap = nswindow_content_height_for(w)
            .map(|h| (h * 4096.0).round() as i64)
            .unwrap_or((win_h_fallback as i64).saturating_mul(4096));
        let ov = overlay.borrow().clone();
        let ov_vis = ov.as_ref().is_some_and(|v| v.is_visible());
        let key = (w.width(), w.height(), snap, w.is_visible(), ov_vis);
        if key != last.get() {
            sync_layer_frame_now(&l_tick, w, ov.as_ref());
            last.set(key);
        }
        glib::ControlFlow::Continue
    });
    id
}
