//! CGL pixel-format / context creation + an OpenGL.framework symbol loader for libmpv.
//!
//! Type aliases come from `objc2_open_gl` so that the `RhinoMpvGlLayer` overrides can
//! match Apple's exact Objective-C type encoding (otherwise objc2's runtime check rejects
//! the override at class registration time).

// Apple deprecated OpenGL on macOS but mpv still uses it on this platform.
#![allow(deprecated)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::NonNull;

pub use objc2_open_gl::{CGLContextObj, CGLPixelFormatObj};
use objc2_open_gl::{
    CGLChoosePixelFormat, CGLContextEnable, CGLContextParameter, CGLCreateContext,
    CGLDestroyContext, CGLDestroyPixelFormat, CGLEnable, CGLError, CGLOpenGLProfile,
    CGLPixelFormatAttribute, CGLSetCurrentContext, CGLSetParameter,
};

#[link(name = "System", kind = "dylib")]
extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

const RTLD_LAZY: c_int = 0x1;

/// Build a CGL pixel format compatible with mpv's GPU renderer (3.2 Core preferred,
/// legacy as a last resort), then create the matching context with vsync on and the
/// multi-threaded GL engine enabled (matching IINA).
pub fn make_pixel_format_and_context() -> Result<(CGLPixelFormatObj, CGLContextObj), String> {
    let pix = choose_pixel_format(CGLOpenGLProfile::CGLOGLPVersion_3_2_Core)
        .or_else(|_| choose_pixel_format(CGLOpenGLProfile::CGLOGLPVersion_Legacy))?;
    let ctx = create_context(pix).map_err(|e| {
        unsafe { CGLDestroyPixelFormat(pix) };
        e
    })?;
    Ok((pix, ctx))
}

fn choose_pixel_format(profile: CGLOpenGLProfile) -> Result<CGLPixelFormatObj, String> {
    // Layout: [key, value, key, key, key, key, key, 0]. Single-attribute keys take no value.
    let attrs: [CGLPixelFormatAttribute; 8] = [
        CGLPixelFormatAttribute::CGLPFAOpenGLProfile,
        CGLPixelFormatAttribute(profile.0),
        CGLPixelFormatAttribute::CGLPFAAccelerated,
        CGLPixelFormatAttribute::CGLPFADoubleBuffer,
        CGLPixelFormatAttribute::CGLPFABackingStore,
        CGLPixelFormatAttribute::CGLPFAAllowOfflineRenderers,
        CGLPixelFormatAttribute::CGLPFASupportsAutomaticGraphicsSwitching,
        CGLPixelFormatAttribute(0),
    ];
    let mut pix: CGLPixelFormatObj = std::ptr::null_mut();
    let mut npix: c_int = 0;
    let err = unsafe {
        CGLChoosePixelFormat(
            NonNull::new_unchecked(attrs.as_ptr() as *mut _),
            NonNull::from(&mut pix),
            NonNull::from(&mut npix),
        )
    };
    if err != CGLError::NoError || pix.is_null() {
        return Err(format!("CGLChoosePixelFormat failed: {}", err.0));
    }
    Ok(pix)
}

fn create_context(pix: CGLPixelFormatObj) -> Result<CGLContextObj, String> {
    let mut ctx: CGLContextObj = std::ptr::null_mut();
    let err = unsafe {
        CGLCreateContext(pix, std::ptr::null_mut(), NonNull::from(&mut ctx))
    };
    if err != CGLError::NoError || ctx.is_null() {
        return Err(format!("CGLCreateContext failed: {}", err.0));
    }
    let one: c_int = 1;
    unsafe {
        let _ = CGLSetParameter(ctx, CGLContextParameter::CGLCPSwapInterval, NonNull::from(&one));
        let _ = CGLEnable(ctx, CGLContextEnable::CGLCEMPEngine);
        let _ = CGLSetCurrentContext(ctx);
    }
    Ok(ctx)
}

/// Free a CGL context + pixel format pair created by [`make_pixel_format_and_context`].
pub fn destroy(pix: CGLPixelFormatObj, ctx: CGLContextObj) {
    unsafe {
        if !ctx.is_null() {
            CGLDestroyContext(ctx);
        }
        if !pix.is_null() {
            CGLDestroyPixelFormat(pix);
        }
    }
}

/// `dlopen` the OpenGL framework once and resolve symbols on demand for the libmpv
/// `get_proc_address` callback. Required so `mpv_render_context` can find `glXxx` entry
/// points without us linking to `libGL` (which doesn't exist on macOS).
pub struct GlSymbolLoader {
    handle: *mut c_void,
}

unsafe impl Send for GlSymbolLoader {}
unsafe impl Sync for GlSymbolLoader {}

impl GlSymbolLoader {
    pub fn open() -> Result<Self, String> {
        let path = CString::new("/System/Library/Frameworks/OpenGL.framework/OpenGL")
            .map_err(|e| e.to_string())?;
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
        if handle.is_null() {
            return Err("dlopen OpenGL.framework failed".into());
        }
        Ok(Self { handle })
    }

    pub fn lookup(&self, name: &str) -> *mut c_void {
        let Ok(c) = CString::new(name) else {
            return std::ptr::null_mut();
        };
        unsafe { dlsym(self.handle, c.as_ptr()) }
    }
}
