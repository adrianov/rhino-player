//! `CAOpenGLLayer` subclass that hosts mpv's render context on macOS.
//!
//! Mirrors IINA's `iina/ViewLayer.swift`:
//! - we own the [`CGLContextObj`] / [`CGLPixelFormatObj`] up front and override
//!   `copyCGLPixelFormat:` / `copyCGLContext:` to hand the same pair to AppKit;
//! - rendering is driven from a background `DispatchQueue` (see [`super::macos_video_attach`]),
//!   so AppKit's nested run loops (menu / popover tracking) cannot stall video — the layer
//!   commits frames via an explicit `CATransaction` and the WindowServer composites them
//!   regardless of what mode the main thread is in.
//!
//! The actual mpv `render` call is supplied by the **owner** through [`set_draw_callback`]
//! so this module stays free of mpv types — keeping the FFI surface small and testable.

// Apple deprecated CAOpenGLLayer + the whole OpenGL stack on macOS, but mpv still binds to
// OpenGL on this platform; mute the platform-deprecation noise across this single module.
#![allow(deprecated)]

use std::cell::Cell;
use std::os::raw::c_int;
use std::sync::Mutex;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, msg_send};
use objc2_core_video::CVTimeStamp;
use objc2_foundation::NSObjectProtocol;

// `CFTimeInterval` is a `c_double` (== `f64`) — use the primitive directly so we can
// keep `objc2_core_foundation` out of the dep tree explicitly.
type CFTimeInterval = f64;
use objc2_quartz_core::{CALayer, CAOpenGLLayer};

use super::macos_video_cgl::{CGLContextObj, CGLPixelFormatObj};

const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8ca6;
const GL_VIEWPORT: u32 = 0x0ba2;

#[link(name = "OpenGL", kind = "framework")]
extern "C" {
    fn glGetIntegerv(pname: u32, params: *mut c_int);
    fn glFlush();
}

/// Closure invoked from inside `drawInCGLContext:` with the live `(fbo, width, height)`.
///
/// `Send + Sync + 'static` — the layer schedules display from a background queue, so the
/// closure can also be called there.
pub type DrawCallback = Box<dyn Fn(c_int, c_int, c_int) + Send + Sync>;

pub struct LayerIvars {
    pub cgl_pixel_format: CGLPixelFormatObj,
    pub cgl_context: CGLContextObj,
    pub draw_cb: Mutex<Option<DrawCallback>>,
    pub fbo: Cell<c_int>,
}

// SAFETY: pointers are immutable after init; `draw_cb` is `Mutex`-guarded; `fbo` is only
// touched from inside `drawInCGLContext:` which CALayer serializes per-instance.
unsafe impl Send for LayerIvars {}
unsafe impl Sync for LayerIvars {}

define_class!(
    /// `RhinoMpvGlLayer` — see module docs.
    #[unsafe(super(CAOpenGLLayer, CALayer))]
    #[name = "RhinoMpvGlLayer"]
    #[ivars = LayerIvars]
    pub struct RhinoMpvGlLayer;

    unsafe impl NSObjectProtocol for RhinoMpvGlLayer {}

    impl RhinoMpvGlLayer {
        #[unsafe(method(copyCGLPixelFormatForDisplayMask:))]
        fn copy_cgl_pixel_format(&self, _mask: u32) -> CGLPixelFormatObj {
            self.ivars().cgl_pixel_format
        }

        #[unsafe(method(copyCGLContextForPixelFormat:))]
        fn copy_cgl_context(&self, _pf: CGLPixelFormatObj) -> CGLContextObj {
            self.ivars().cgl_context
        }

        // We retain the context / pixel format for the layer's whole lifetime — releasing
        // them when AppKit hands them back would invalidate the mpv render context.
        #[unsafe(method(releaseCGLContext:))]
        fn release_cgl_context(&self, _ctx: CGLContextObj) {}

        #[unsafe(method(releaseCGLPixelFormat:))]
        fn release_cgl_pixel_format(&self, _pf: CGLPixelFormatObj) {}

        #[unsafe(method(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn can_draw(
            &self,
            _ctx: CGLContextObj,
            _pf: CGLPixelFormatObj,
            _t: CFTimeInterval,
            _ts: *const CVTimeStamp,
        ) -> bool {
            // We drive frames imperatively via `display_now` from a CVDisplayLink (background
            // thread). Always say yes — the displayLink only triggers display when mpv has
            // produced a new frame, so this never wastes work.
            true
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw_in_cgl(
            &self,
            _ctx: CGLContextObj,
            _pf: CGLPixelFormatObj,
            _t: CFTimeInterval,
            _ts: *const CVTimeStamp,
        ) {
            let mut fbo: c_int = 0;
            unsafe { glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo) };
            self.ivars().fbo.set(fbo);
            let mut viewport: [c_int; 4] = [0, 0, 0, 0];
            unsafe { glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr()) };
            let w = viewport[2];
            let h = viewport[3];
            let cb_slot = self.ivars().draw_cb.lock();
            if let Ok(slot) = cb_slot {
                if let Some(cb) = slot.as_ref() {
                    cb(fbo, w, h);
                }
            }
            unsafe { glFlush() };
        }
    }
);

impl RhinoMpvGlLayer {
    pub fn new(
        cgl_pixel_format: CGLPixelFormatObj,
        cgl_context: CGLContextObj,
    ) -> Retained<Self> {
        let this = Self::alloc().set_ivars(LayerIvars {
            cgl_pixel_format,
            cgl_context,
            draw_cb: Mutex::new(None),
            fbo: Cell::new(0),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        // Asynchronous mode: AppKit drives `drawInCGLContext:` from its CA timer plus
        // `setNeedsDisplay` notifications. The CVDisplayLink in `macos_video_displaylink`
        // calls `setNeedsDisplay` from a background thread, which AppKit treats as a
        // commit hint and schedules the next draw — even during AppKit modal tracking,
        // because CA's display path runs on `kCFRunLoopCommonModes`.
        this.setAsynchronous(true);
        this.setOpaque(true);
        this.setNeedsDisplayOnBoundsChange(true);
        this
    }

    /// Replace the draw callback. Pass `None` to clear (used during teardown).
    pub fn set_draw_callback(&self, cb: Option<DrawCallback>) {
        if let Ok(mut slot) = self.ivars().draw_cb.lock() {
            *slot = cb;
        }
    }

    /// Mark the layer dirty from the CVDisplayLink callback. With `asynchronous = true`
    /// AppKit will read this on the next CA commit (which runs in
    /// `kCFRunLoopCommonModes` — including AppKit modal tracking) and call
    /// `drawInCGLContext:` on its own scheduling thread. Cheap; safe from any thread.
    pub fn display_now(&self) {
        unsafe {
            let _: () = msg_send![self, setNeedsDisplay];
        }
    }

    /// Update the layer's backing scale (`contentsScale`) so the FBO mpv renders into
    /// matches the screen's pixel density. Without this, on Retina the layer renders
    /// at point resolution and AppKit upscales — text (subtitles, OSD) goes blurry.
    /// Call on widget realize and on every `notify::scale-factor`.
    pub fn set_backing_scale(&self, scale: f64) {
        let s = if scale > 0.0 { scale } else { 1.0 };
        unsafe {
            let _: () = msg_send![self, setContentsScale: s];
        }
    }
}

/// Convenience: upcast to a generic `CALayer` so [`super::macos_video_view`] can attach it
/// to an `NSView`.
pub fn as_calayer(layer: &Retained<RhinoMpvGlLayer>) -> Retained<CALayer> {
    let raw: *const CALayer = (Retained::as_ptr(layer) as *const AnyObject).cast();
    unsafe { Retained::retain(raw as *mut CALayer).expect("layer pointer must be live") }
}
