//! AppKit attachment steps used when a [`super::NativeVideoSurface`] is installed:
//! fetch the window's contentView backing layer, insert the video layer at the bottom of
//! gdk-macos's compositing stack, mirror monitor-scale changes onto the layer, and hide
//! the video layer while an overlay widget is visible.

#![allow(deprecated)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use glib::object::{IsA, ObjectExt};
use gtk::prelude::WidgetExt;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
use objc2_quartz_core::CALayer;

use super::super::macos_video_cgl::{
    make_pixel_format_and_context, CGLContextObj, CGLPixelFormatObj, GlSymbolLoader,
};
use super::super::macos_video_displaylink::DriverStateHandle;
use super::super::macos_video_layer::{as_calayer, RhinoMpvGlLayer};
use super::super::macos_video_layer_frame::{sync_layer_frame_now, wire_sizer_resync};

/// Native pieces built + attached before any GTK signal wiring starts.
pub(super) struct AttachedLayers {
    pub(super) pixel_format: CGLPixelFormatObj,
    pub(super) context: CGLContextObj,
    pub(super) gl_loader: Arc<GlSymbolLoader>,
    pub(super) layer: Retained<RhinoMpvGlLayer>,
    pub(super) parent_layer: Retained<CALayer>,
}

/// Create the CGL context + mpv GL layer and insert it below gdk-macos's GTK rendering.
pub(super) fn attach_native_layers(
    window: &NSWindow,
    backing_scale: f64,
) -> Result<AttachedLayers, String> {
    let render = make_gl_stack(backing_scale)?;
    let parent_layer = parent_layer_of(window)?;
    insert_below_gtk_sublayers(&parent_layer, &as_calayer(&render.layer));
    Ok(AttachedLayers {
        pixel_format: render.pixel_format,
        context: render.context,
        gl_loader: render.gl_loader,
        layer: render.layer,
        parent_layer,
    })
}

/// CGL pixel format + context, symbol loader, and the mpv GL layer itself.
struct GlStack {
    pixel_format: CGLPixelFormatObj,
    context: CGLContextObj,
    gl_loader: Arc<GlSymbolLoader>,
    layer: Retained<RhinoMpvGlLayer>,
}

fn make_gl_stack(backing_scale: f64) -> Result<GlStack, String> {
    let (pix, ctx) = make_pixel_format_and_context()?;
    let gl_loader = Arc::new(GlSymbolLoader::open()?);
    let layer = RhinoMpvGlLayer::new(pix, ctx);
    layer.set_backing_scale(backing_scale);
    Ok(GlStack {
        pixel_format: pix,
        context: ctx,
        gl_loader,
        layer,
    })
}

fn parent_layer_of(window: &NSWindow) -> Result<Retained<CALayer>, String> {
    let content_view: Retained<NSView> = unsafe {
        let cv: *mut NSView = msg_send![window, contentView];
        Retained::retain(cv).ok_or("contentView is nil")?
    };

    // Make sure the contentView is layer-backed (gdk-macos already does this, but
    // belt-and-braces). Then insert our layer as a direct sublayer with a high
    // zPosition so it's composited above gdk's GTK rendering.
    let parent_layer: Retained<CALayer> = unsafe {
        let _: () = msg_send![&*content_view, setWantsLayer: true];
        let cv_layer: *mut CALayer = msg_send![&*content_view, layer];
        Retained::retain(cv_layer).ok_or("contentView.layer is nil after setWantsLayer")?
    };
    Ok(parent_layer)
}

fn insert_below_gtk_sublayers(parent_layer: &CALayer, our_calayer: &CALayer) {
    unsafe {
        // Insert at the BOTTOM of the contentView's sublayer stack and skip
        // `setZPosition:` so gdk-macos's GTK rendering sublayer (which carries the
        // header / bottom bar / GLArea) composites *above* us. The GTK GLArea is made
        // transparent by [`super::macos_video_bundle::install_transparent_glarea`]
        // (`background-color: transparent` + an alpha-0 GL clear in the render
        // callback) so the video region of gdk's sublayer is alpha=0 and our layer
        // shows through, while the bars stay opaque on top.
        let _: () = msg_send![parent_layer, insertSublayer: our_calayer, atIndex: 0u32];
    }
}

/// Track Retina / non-Retina monitor changes so the FBO matches actual pixels.
pub(super) fn wire_backing_scale_tracking(
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
) {
    let l_scale = layer.clone();
    sizer_widget.connect_local("notify::scale-factor", false, move |args| {
        if let Ok(w) = args[0].get::<gtk::Widget>() {
            l_scale.set_backing_scale(w.scale_factor() as f64);
        }
        None
    });
}

type OverlayCell = Rc<RefCell<Option<gtk::Widget>>>;

/// While `w` is visible, [`sync_layer_frame_now`] keeps the video layer hidden so the
/// GTK overlay (recent grid, etc.) shows through.
pub(super) fn connect_overlay_visibility(
    w: &gtk::Widget,
    sizer_widget: &gtk::Widget,
    layer: &Retained<RhinoMpvGlLayer>,
    overlay: &OverlayCell,
    repaint: &Arc<DriverStateHandle>,
) {
    let l_vis = layer.clone();
    let s_vis = sizer_widget.clone();
    let ov_vis = overlay.clone();
    let r_vis = repaint.clone();
    w.connect_local("notify::visible", false, move |_| {
        let ov = ov_vis.borrow().clone();
        sync_layer_frame_now(&l_vis, &s_vis, ov.as_ref(), Some(r_vis.as_ref()));
        None
    });
}

/// GTK-side wiring started right after the native layer is attached.
pub(super) struct RenderSession {
    pub(super) display_link: super::super::macos_video_displaylink::DisplayLinkDriver,
    pub(super) redraw_handle: Arc<DriverStateHandle>,
    pub(super) overlay: OverlayCell,
    pub(super) sizer_handler: glib::SignalHandlerId,
}

/// Start the displayLink, do the first frame sync, and wire the sizer resync signals.
pub(super) fn start_session<W: IsA<gtk::Widget>>(
    native: &AttachedLayers,
    sizer_widget: &gtk::Widget,
    sizer: &W,
) -> Result<RenderSession, String> {
    let overlay: OverlayCell = Rc::new(RefCell::new(None));
    let (display_link, redraw_handle) =
        super::super::macos_video_displaylink::DisplayLinkDriver::install(native.layer.clone())?;
    sync_layer_frame_now(&native.layer, sizer, None, Some(redraw_handle.as_ref()));
    Ok(RenderSession {
        display_link,
        sizer_handler: wire_sizer_resync(
            sizer_widget,
            native.layer.clone(),
            overlay.clone(),
            redraw_handle.clone(),
        ),
        redraw_handle,
        overlay,
    })
}
