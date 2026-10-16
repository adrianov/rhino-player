//! `CAOpenGLLayer` subclass that hosts mpv's render context on macOS.
//!
//! Mirrors IINA's `iina/ViewLayer.swift`:
//! - we own the [`CGLContextObj`] / [`CGLPixelFormatObj`] up front and override
//!   `copyCGLPixelFormat:` / `copyCGLContext:` to hand the same pair to AppKit;
//! - rendering is driven from the `CVDisplayLink` thread (see [`super::macos_video_displaylink`]),
//!   so neither AppKit's nested run loops (menu / popover tracking) nor GTK main-thread work
//!   can stall video — the layer commits frames via an explicit `CATransaction` and the
//!   WindowServer composites them regardless of what the main thread is doing.
//!
//! The actual mpv `render` call is supplied by the **owner** through [`set_draw_callback`]
//! so this module stays free of mpv types — keeping the FFI surface small and testable.

// Apple deprecated CAOpenGLLayer + the whole OpenGL stack on macOS, but mpv still binds to
// OpenGL on this platform; mute the platform-deprecation noise across this single module.
#![allow(deprecated)]

use std::os::raw::c_int;
use std::sync::Mutex;

use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, AnyThread, DefinedClass};
use objc2_core_video::CVTimeStamp;
use objc2_foundation::NSObjectProtocol;

// `CFTimeInterval` is a `c_double` (== `f64`) — use the primitive directly so we can
// keep `objc2_core_foundation` out of the dep tree explicitly.
type CFTimeInterval = f64;
use objc2_quartz_core::{CALayer, CAOpenGLLayer, CATransaction};

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
/// `Send + Sync + 'static` — called on the CVDisplayLink thread, and on the main thread
/// when CA redraws after a bounds change.
pub type DrawCallback = Box<dyn Fn(c_int, c_int, c_int) + Send + Sync>;

pub struct LayerIvars {
    pub cgl_pixel_format: CGLPixelFormatObj,
    pub cgl_context: CGLContextObj,
    pub draw_cb: Mutex<Option<DrawCallback>>,
    /// Serializes whole `display` passes: CA's own GL setup / flush on the shared context
    /// runs outside `drawInCGLContext:`, and main (bounds change) may race the link thread.
    display_lock: Mutex<()>,
}

// SAFETY: pointers are immutable after init; `draw_cb` / `display_lock` are `Mutex`-guarded.
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
            // Frames are driven imperatively via `display_now` from the CVDisplayLink thread
            // only when mpv has produced a new frame, so always say yes.
            true
        }

        #[unsafe(method(display))]
        fn display_locked(&self) {
            let _pass = self.ivars().display_lock.lock();
            unsafe {
                let _: () = msg_send![super(self), display];
            }
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw_in_cgl(
            &self,
            _ctx: CGLContextObj,
            _pf: CGLPixelFormatObj,
            _t: CFTimeInterval,
            _ts: *const CVTimeStamp,
        ) {
            // Held for the whole draw so teardown cannot clear the callback mid-render.
            let Ok(slot) = self.ivars().draw_cb.lock() else {
                return;
            };
            let mut fbo: c_int = 0;
            unsafe { glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo) };
            let mut viewport: [c_int; 4] = [0, 0, 0, 0];
            unsafe { glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr()) };
            if let Some(cb) = slot.as_ref() {
                cb(fbo, viewport[2], viewport[3]);
            }
            unsafe { glFlush() };
        }
    }
);

impl RhinoMpvGlLayer {
    pub fn new(cgl_pixel_format: CGLPixelFormatObj, cgl_context: CGLContextObj) -> Retained<Self> {
        let this = Self::alloc().set_ivars(LayerIvars {
            cgl_pixel_format,
            cgl_context,
            draw_cb: Mutex::new(None),
            display_lock: Mutex::new(()),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        // Synchronous mode: frames are drawn by `display_now` on the CVDisplayLink thread.
        // Asynchronous mode would make CA call `drawInCGLContext:` on the main run loop, so
        // any GTK main-thread stall (preview load, thumbnails, layout) froze the video.
        this.setAsynchronous(false);
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

    /// Draw one frame on the calling (CVDisplayLink) thread and commit it in an explicit
    /// `CATransaction`, so the WindowServer composites it without waiting for the main thread.
    pub fn display_now(&self) {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.display();
        CATransaction::commit();
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
