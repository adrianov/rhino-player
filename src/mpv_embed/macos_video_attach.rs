//! Attach the native video [`NSView`] to the GTK window's `NSWindow`, mirror a GTK
//! widget's allocation onto its frame, and own the background `DispatchQueue` that drives
//! mpv's render path so AppKit modal tracking can never starve it.
//!
//! Public entry point: [`NativeVideoSurface::install`]. The returned guard holds the
//! NSView, the dispatch queue, and the size-tracking signal handler — drop it (or call
//! [`NativeVideoSurface::detach`]) to tear everything down.

#![allow(deprecated)]

use std::sync::Arc;

use gdk4_macos::MacosSurface;
use gdk4_macos::prelude::Cast;
use glib::SignalHandlerId;
use glib::object::{IsA, ObjectExt};
use gtk::prelude::{NativeExt, WidgetExt, WidgetExtManual};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSView, NSWindow};

use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_quartz_core::{CALayer, CATransaction};

use super::macos_video_cgl::{
    self, CGLContextObj, CGLPixelFormatObj, GlSymbolLoader, make_pixel_format_and_context,
};
use super::macos_video_displaylink::{DisplayLinkDriver, DriverStateHandle};
use super::macos_video_layer::{DrawCallback, RhinoMpvGlLayer, as_calayer};

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

    /// Register an "overlay" widget — when it becomes visible the video layer hides so
    /// the GTK overlay (recent grid, etc.) shows through. The tick callback installed
    /// by [`wire_sizer_resync`] re-checks `overlay.is_visible()` every frame, and
    /// `notify::visible` triggers an immediate resync.
    pub fn watch_overlay<W: IsA<gtk::Widget>>(&self, widget: &W) {
        let w = widget.clone().upcast::<gtk::Widget>();
        *self.overlay.borrow_mut() = Some(w.clone());
        let Some(sizer_widget) = self.sizer_widget.clone() else { return; };
        let layer = self.layer.clone();
        let overlay = self.overlay.clone();
        w.connect_local("notify::visible", false, move |_| {
            let ov = overlay.borrow().clone();
            sync_layer_frame_now(&layer, &sizer_widget, ov.as_ref());
            None
        });
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

impl Drop for NativeVideoSurface {
    fn drop(&mut self) {
        self.detach();
        // Layer + view are released by their `Retained`s. CGL context / pixel format are not
        // refcounted by Cocoa — release them explicitly. The layer no longer touches them
        // after `set_draw_callback(None)` above.
        macos_video_cgl::destroy(self.pixel_format, self.context);
    }
}

/// Find the NSWindow that hosts `widget`. Returns `None` before the GtkWindow is realized.
fn nswindow_for<W: IsA<gtk::Widget>>(widget: &W) -> Option<Retained<NSWindow>> {
    let surface = widget.native()?.surface()?;
    let macos = surface.downcast::<MacosSurface>().ok()?;
    let ptr = macos.native() as *mut NSWindow;
    if ptr.is_null() {
        return None;
    }
    unsafe { Retained::retain(ptr) }
}

fn translate_to_window<W: IsA<gtk::Widget>>(widget: &W, win: &gtk::Window) -> Option<(f64, f64)> {
    widget
        .compute_point(win, &gtk::graphene::Point::new(0.0, 0.0))
        .map(|p| (p.x() as f64, p.y() as f64))
}

/// Mirror the `sizer` widget's allocation + visibility onto the layer every frame. The
/// tick callback short-circuits on no-op frames, plus an initial pass on `notify::root`
/// and `connect_map` covers first attach + re-show after hide. We also re-sync on
/// `notify::visible` so toggling the recent grid hides the video layer immediately.
fn wire_sizer_resync(
    sizer_widget: &gtk::Widget,
    layer: Retained<RhinoMpvGlLayer>,
    overlay: std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>>,
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
    let last = std::cell::Cell::new((0i32, 0i32, 0i32, false, false));
    sizer_widget.add_tick_callback(move |w, _| {
        let win_h = w.root().and_then(|r| r.downcast::<gtk::Window>().ok()).map(|win| win.height()).unwrap_or(0);
        let ov = overlay.borrow().clone();
        let ov_vis = ov.as_ref().is_some_and(|v| v.is_visible());
        let key = (w.width(), w.height(), win_h, w.is_visible(), ov_vis);
        if key != last.get() {
            sync_layer_frame_now(&l_tick, w, ov.as_ref());
            last.set(key);
        }
        glib::ControlFlow::Continue
    });
    id
}

fn sync_layer_frame_now<W: IsA<gtk::Widget>>(
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
    // Hide the layer when the recent grid (or any other watched overlay) is shown so
    // GTK's overlay paints through; otherwise our high-zPosition layer covers it.
    let overlay_shown = overlay.is_some_and(|w| w.is_visible());
    let visible = sizer.is_visible() && sizer.is_mapped() && !overlay_shown;
    let w = (sizer.width() as f64).max(1.0);
    let h = (sizer.height() as f64).max(1.0);
    // Use the NSWindow contentView's actual height (not GTK's `win.height()`) — they
    // can differ by a fraction of a point on macOS, leaving a thin uncovered strip
    // above the video where the chrome rounds against the layer.
    let win_h = nswindow_content_height_for(sizer).unwrap_or_else(|| window.height() as f64);
    // GTK = top-left origin; AppKit/CA = bottom-left. Flip Y.
    let ns_y = win_h - y - h;
    let frame = NSRect::new(NSPoint::new(x, ns_y), NSSize::new(w, h));
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h));
    // Suppress the implicit 0.25 s CA animation that would otherwise interpolate every
    // resize step from the GTK frame clock and look like a stutter against gdk's chrome.
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    unsafe {
        let _: () = msg_send![layer, setFrame: frame];
        let _: () = msg_send![layer, setBounds: bounds];
        let _: () = msg_send![layer, setHidden: !visible];
    }
    CATransaction::commit();
}

/// Returns the height (in points) of the NSWindow's contentView for this widget.
/// Reading directly from AppKit avoids the off-by-rounding mismatch with `gtk::Window::height()`.
fn nswindow_content_height_for<W: IsA<gtk::Widget>>(sizer: &W) -> Option<f64> {
    let win = nswindow_for(sizer)?;
    unsafe {
        let cv: *mut NSView = msg_send![&*win, contentView];
        if cv.is_null() {
            return None;
        }
        let frame: NSRect = msg_send![cv, frame];
        Some(frame.size.height)
    }
}

/// Create the native surface, attach as a subview of the NSWindow's `contentView`, and
/// start mirroring `sizer`'s allocation onto the view's frame.
///
/// Must be called on the main thread.
pub fn install<W: IsA<gtk::Widget>>(sizer: &W) -> Result<NativeVideoSurface, String> {
    let _ = MainThreadMarker::new().ok_or("install must run on the main thread")?;
    let window = nswindow_for(sizer).ok_or("NSWindow not realized for video sizer")?;
    let (pix, ctx) = make_pixel_format_and_context()?;
    let gl_loader = Arc::new(GlSymbolLoader::open()?);
    let layer = RhinoMpvGlLayer::new(pix, ctx);

    let content_view: Retained<NSView> = unsafe {
        let cv: *mut NSView = msg_send![&*window, contentView];
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

    let overlay: std::rc::Rc<std::cell::RefCell<Option<gtk::Widget>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    sync_layer_frame_now(&layer, sizer, None);
    layer.set_backing_scale(sizer.scale_factor() as f64);
    let our_calayer = as_calayer(&layer);
    unsafe {
        // Insert at the BOTTOM of the contentView's sublayer stack and skip
        // `setZPosition:` so gdk-macos's GTK rendering sublayer (which carries the
        // header / bottom bar / GLArea) composites *above* us. The GTK GLArea is made
        // transparent by [`super::macos_video_bundle::install_transparent_glarea`]
        // (`background-color: transparent` + an alpha-0 GL clear in the render
        // callback) so the video region of gdk's sublayer is alpha=0 and our layer
        // shows through, while the bars stay opaque on top.
        let _: () = msg_send![&*parent_layer, insertSublayer: &*our_calayer, atIndex: 0u32];
    }

    let sizer_widget = sizer.clone().upcast::<gtk::Widget>();
    let id = wire_sizer_resync(&sizer_widget, layer.clone(), overlay.clone());

    // Track Retina / non-Retina monitor changes so the FBO matches actual pixels.
    let l_scale = layer.clone();
    sizer_widget.connect_local("notify::scale-factor", false, move |args| {
        if let Ok(w) = args[0].get::<gtk::Widget>() {
            l_scale.set_backing_scale(w.scale_factor() as f64);
        }
        None
    });

    let (display_link, redraw_handle) = DisplayLinkDriver::install(layer.clone())?;

    Ok(NativeVideoSurface {
        layer,
        parent_layer,
        display_link: Some(display_link),
        redraw_handle,
        pixel_format: pix,
        context: ctx,
        gl_loader,
        sizer: Some(id),
        sizer_widget: Some(sizer_widget),
        overlay,
    })
}

