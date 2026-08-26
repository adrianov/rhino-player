//! Attach the native video [`NSView`] to the GTK window's `NSWindow`, mirror a GTK
//! widget's allocation onto its frame, and own the background `DispatchQueue` that drives
//! mpv's render path so AppKit modal tracking can never starve it.
//!
//! Public entry point: [`NativeVideoSurface::install`]. The returned guard holds the
//! NSView, the dispatch queue, and the size-tracking signal handler — drop it (or call
//! [`NativeVideoSurface::detach`]) to tear everything down.

#![allow(deprecated)]

use std::sync::Arc;

use glib::object::{IsA, ObjectExt};
use glib::SignalHandlerId;
use gtk::prelude::{Cast, WidgetExt};
use objc2::rc::Retained;
use objc2::{msg_send, MainThreadMarker};

use crate::macos_window::nswindow_for_widget;

use objc2_quartz_core::CALayer;

use super::macos_video_cgl::{self, CGLContextObj, CGLPixelFormatObj, GlSymbolLoader};
use super::macos_video_displaylink::{DisplayLinkDriver, DriverStateHandle};
use super::macos_video_layer::{DrawCallback, RhinoMpvGlLayer};
use super::macos_video_layer_frame::sync_layer_frame_now;

mod nswindow_attach;

use self::nswindow_attach::{
    attach_native_layers, connect_overlay_visibility, start_session, wire_backing_scale_tracking,
    AttachedLayers, RenderSession,
};

/// Public handle returned from [`install`]. Drops everything in order on release.
///
/// Frames are driven by a [`DisplayLinkDriver`] (CVDisplayLink running on a dedicated
/// kernel thread). mpv's update callback flips a pending bit through
/// [`DriverStateHandle::mark_pending`]; the displayLink consumes it on the next vsync,
/// holding the CGL context lock while it asks the layer to render. AppKit modal tracking
/// on the GTK main thread cannot stall any of this — the displayLink thread is outside
/// CFRunLoop entirely.
///
/// The layer is inserted as a **direct sublayer** of the NSWindow's contentView's
/// `layer`, not as the backing layer of an NSView. gdk-macos renders GTK widgets
/// straight into the contentView's layer (no NSView subviews), so adding our layer to
/// the same CALayer hierarchy is the only way to get composited.
pub struct NativeVideoSurface {
    layer: Retained<RhinoMpvGlLayer>,
    parent_layer: Retained<CALayer>,
    /// Hold this so it's dropped (stop + detach callback) before `layer`/CGL context.
    display_link: Option<DisplayLinkDriver>,
    /// Cheap clone for the mpv update callback.
    redraw_handle: Arc<DriverStateHandle>,
    pixel_format: CGLPixelFormatObj,
    context: CGLContextObj,
    gl_loader: Arc<GlSymbolLoader>,
    sizer: Option<SignalHandlerId>,
    sizer_widget: Option<gtk::Widget>,
    /// Optional GTK widget whose `is_visible()` toggles the video layer off (e.g. the
    /// recent grid overlay). Wired by [`watch_overlay`].
    overlay: std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>>,
}

impl NativeVideoSurface {
    pub(crate) fn pause_cv_display_link(&self) {
        if let Some(ref dl) = self.display_link {
            let _ = dl.set_cv_running(false);
        }
    }

    pub(crate) fn resume_cv_display_link(&self) {
        if let Some(ref dl) = self.display_link {
            let _ = dl.set_cv_running(true);
        }
    }

    /// Symbol loader for libmpv's `get_proc_address` callback.
    pub fn gl_loader(&self) -> Arc<GlSymbolLoader> {
        Arc::clone(&self.gl_loader)
    }

    /// Install / replace the per-frame draw callback. mpv's render call goes here.
    pub fn set_draw_callback(&self, cb: Option<DrawCallback>) {
        self.layer.set_draw_callback(cb);
    }

    /// Cheap clone of the displayLink handle — give this to mpv's update callback so it
    /// can mark a frame pending from any thread.
    pub fn redraw_handle(&self) -> Arc<DriverStateHandle> {
        Arc::clone(&self.redraw_handle)
    }

    /// Re-run layer frame + visibility sync (continue-grid warm `loadfile`, resize, etc.).
    pub fn resync_layer_frame(&self) {
        let Some(sizer_widget) = self.sizer_widget.clone() else {
            return;
        };
        let ov = self.overlay.borrow().clone();
        sync_layer_frame_now(
            &self.layer,
            &sizer_widget,
            ov.as_ref(),
            Some(self.redraw_handle.as_ref()),
        );
    }

    /// Keep the native mpv layer under gdk-macos's GTK compositing sublayer (post-resize).
    pub fn repin_below_gtk_compositing(&self) {
        super::macos_video_layer_frame::pin_video_layer_below_gtk(&self.layer);
    }

    /// Register an "overlay" widget — when it becomes visible the video layer hides so
    /// the GTK overlay (recent grid, etc.) shows through. The tick callback installed
    /// by [`wire_sizer_resync`] re-checks `overlay.is_visible()` every frame, and
    /// `notify::visible` triggers an immediate resync.
    pub fn watch_overlay<W: IsA<gtk::Widget>>(&self, widget: &W) {
        let w = widget.clone().upcast::<gtk::Widget>();
        *self.overlay.borrow_mut() = Some(w.clone());
        let Some(sizer_widget) = self.sizer_widget.clone() else {
            return;
        };
        connect_overlay_visibility(
            &w,
            &sizer_widget,
            &self.layer,
            &self.overlay,
            &self.redraw_handle,
        );
    }

    /// Detach the layer from contentView, stop the displayLink, drop the size-tracking
    /// signal, and clear the draw callback so any in-flight render becomes a no-op.
    pub fn detach(&mut self) {
        self.display_link.take();
        if let (Some(id), Some(w)) = (self.sizer.take(), self.sizer_widget.take()) {
            w.disconnect(id);
        }
        unsafe {
            let _: () = msg_send![&*self.layer, removeFromSuperlayer];
        }
        self.layer.set_draw_callback(None);
        let _ = &self.parent_layer;
    }
}

impl NativeVideoSurface {
    /// Assemble the surface from its attached native pieces and the running session.
    fn assemble(native: AttachedLayers, session: RenderSession, sizer_widget: gtk::Widget) -> Self {
        Self {
            layer: native.layer,
            parent_layer: native.parent_layer,
            display_link: Some(session.display_link),
            redraw_handle: session.redraw_handle,
            pixel_format: native.pixel_format,
            context: native.context,
            gl_loader: native.gl_loader,
            sizer: Some(session.sizer_handler),
            sizer_widget: Some(sizer_widget),
            overlay: session.overlay,
        }
    }
}

impl Drop for NativeVideoSurface {
    fn drop(&mut self) {
        self.detach();
        // Layer + view are released by their `Retained`s. CGL context / pixel format are not
        // refcounted by Cocoa — release them explicitly. The layer no longer touches them
        // after `set_draw_callback(None)` above.
        macos_video_cgl::destroy(self.pixel_format, self.context);
    }
}

/// Create the native surface, attach as a subview of the NSWindow's `contentView`, and
/// start mirroring `sizer`'s allocation onto the view's frame.
///
pub fn install<W: IsA<gtk::Widget>>(sizer: &W) -> Result<NativeVideoSurface, String> {
    let _ = MainThreadMarker::new().ok_or("install must run on the main thread")?;
    let window = nswindow_for_widget(sizer).ok_or("NSWindow not realized for video sizer")?;
    let native = attach_native_layers(&window, sizer.scale_factor() as f64)?;
    let sizer_widget = sizer.clone().upcast::<gtk::Widget>();
    let session = start_session(&native, &sizer_widget, sizer)?;
    wire_backing_scale_tracking(&sizer_widget, &native.layer);
    Ok(NativeVideoSurface::assemble(native, session, sizer_widget))
}
