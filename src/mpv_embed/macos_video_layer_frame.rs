//! Per-frame mirroring from the NSWindow contentView onto a [`RhinoMpvGlLayer`]'s
//! Cocoa frame, plus the GTK signal wiring that drives it. Pulled out of
//! `macos_video_attach.rs` so each module stays under the soft 300-line limit.
//!
//! The video layer is sized to the **contentView layer bounds** (gdk-macos compositing
//! root), not the GTK [`GLArea`] allocation. Mirroring the GLArea alone can leave the
//! native surface inset inside a larger contentView — black margins on all four sides —
//! because gdk-macos's root layer is geometry-flipped and GTK vs AppKit sizes can drift
//! after programmatic resize. Opaque chrome tiles still cover the header / bottom bar.

#![allow(deprecated)]

use glib::object::IsA;
use glib::SignalHandlerId;
use gtk::prelude::WidgetExt;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::NSRect;
use objc2_quartz_core::{CALayer, CATransaction};

use crate::macos_window::nswindow_for_widget;

use super::macos_video_displaylink::DriverStateHandle;
use super::macos_video_layer::RhinoMpvGlLayer;

mod resync_ticker;
mod resync_wiring;

pub(super) type OverlayCell = std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>>;

/// ContentView layer bounds in points (the compositing root we attach under).
pub(super) fn content_view_bounds<W: IsA<gtk::Widget>>(sizer: &W) -> Option<NSRect> {
    let win = nswindow_for_widget(sizer)?;
    unsafe {
        let cv: *mut NSView = msg_send![&*win, contentView];
        if cv.is_null() {
            return None;
        }
        let cv_layer: *mut CALayer = msg_send![cv, layer];
        let bounds: NSRect = if cv_layer.is_null() {
            msg_send![cv, bounds]
        } else {
            msg_send![cv_layer, bounds]
        };
        (bounds.size.width > 0.5 && bounds.size.height > 0.5).then_some(bounds)
    }
}

/// ContentView layer size in points — tick debounce key.
pub(super) fn content_view_size<W: IsA<gtk::Widget>>(sizer: &W) -> Option<(f64, f64)> {
    content_view_bounds(sizer).map(|b| (b.size.width, b.size.height))
}

/// Whether the video layer should be visible: sizer visible+mapped, no overlay shown.
fn target_visible<W: IsA<gtk::Widget>>(sizer: &W, overlay: Option<&gtk::Widget>) -> bool {
    sizer.is_visible() && sizer.is_mapped() && !overlay.is_some_and(|w| w.is_visible())
}

/// Target frame of the video layer in contentView coordinates, plus whether the
/// layer should be visible. Parent (`GdkMacosLayer`) is geometry-flipped; its
/// bounds already cover the player content area in that space.
fn sync_geometry<W: IsA<gtk::Widget>>(
    sizer: &W,
    overlay: Option<&gtk::Widget>,
) -> Option<(NSRect, bool)> {
    let bounds = content_view_bounds(sizer)?;
    // Fill the parent exactly — do not re-derive from GTK GLArea allocation.
    Some((bounds, target_visible(sizer, overlay)))
}

/// Fill the NSWindow contentView — chrome overlays via opaque gdk-macos tiles above this layer.
pub(super) fn sync_layer_frame_now<W: IsA<gtk::Widget>>(
    layer: &RhinoMpvGlLayer,
    sizer: &W,
    overlay: Option<&gtk::Widget>,
    repaint: Option<&DriverStateHandle>,
) {
    let Some((frame, visible)) = sync_geometry(sizer, overlay) else {
        eprintln!(
            "[rhino] video-layer: sync skipped (no contentView size) sizer={}x{} mapped={}",
            sizer.width(),
            sizer.height(),
            sizer.is_mapped()
        );
        return;
    };
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    unsafe {
        // Frame only: `setBounds:` after `setFrame:` can fight geometry-flipped parents.
        let _: () = msg_send![layer, setFrame: frame];
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

/// Mirror the contentView size + sizer visibility onto `layer` every frame. The
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
