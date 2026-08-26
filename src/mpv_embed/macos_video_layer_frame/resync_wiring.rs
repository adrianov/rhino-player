//! GTK signal + tick wiring that drives [`super::sync_layer_frame_now`]. Each connector
//! clones the shared layer / overlay / repaint handles; the tick callback short-circuits
//! no-op frames via change keys.

use std::sync::Arc;

use glib::object::ObjectExt;
use gtk::prelude::WidgetExt;
use objc2::rc::Retained;

use super::super::macos_video_displaylink::DriverStateHandle;
use super::super::macos_video_layer::RhinoMpvGlLayer;

use super::{sync_layer_frame_now, OverlayCell};

/// One resync pass: borrow the current overlay widget and mirror frame + visibility.
pub(super) fn resync_now<W: glib::object::IsA<gtk::Widget>>(
    layer: &Retained<RhinoMpvGlLayer>,
    sizer: &W,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
) {
    let ov = overlay.borrow().clone();
    sync_layer_frame_now(layer, sizer, ov.as_ref(), Some(repaint.as_ref()));
}

/// Re-sync whenever the sizer is re-parented into a new toplevel.
pub(super) fn connect_root(
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
) -> glib::SignalHandlerId {
    let l = layer.clone();
    let s = sizer_widget.clone();
    let ov = overlay.clone();
    let r = repaint.clone();
    sizer_widget.connect_local("notify::root", false, move |_| {
        resync_now(&l, &s, &ov, &r);
        None
    })
}

/// Re-sync when the sizer's visibility flips (video hides/shows immediately).
pub(super) fn connect_visible(
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
) {
    let l = layer.clone();
    let ov = overlay.clone();
    let r = repaint.clone();
    sizer_widget.connect_local("notify::visible", false, move |args| {
        if let Ok(w) = args[0].get::<gtk::Widget>() {
            resync_now(&l, &w, &ov, &r);
        }
        None
    });
}

/// Cover first attach after the sizer is mapped.
pub(super) fn connect_map(
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
) {
    let l = layer.clone();
    let ov = overlay.clone();
    let r = repaint.clone();
    sizer_widget.connect_map(move |w| {
        resync_now(&l, w, &ov, &r);
    });
}

/// Live resize: re-sync on width/height notify.
pub(super) fn connect_size_notify(
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
    prop: &'static str,
) {
    let l = layer.clone();
    let ov = overlay.clone();
    let r = repaint.clone();
    sizer_widget.connect_notify_local(
        Some(prop),
        glib::clone!(
            #[strong]
            l,
            #[strong]
            ov,
            #[strong]
            r,
            move |w, _| {
                resync_now(&l, w, &ov, &r);
            }
        ),
    );
}
