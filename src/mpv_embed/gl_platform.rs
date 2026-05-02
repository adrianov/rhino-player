//! OpenGL symbols for libmpv render: **EGL** + **libGL** on Linux; **dlsym** after GTK loads GL on macOS.

#[cfg(target_os = "linux")]
use libloading::Library;
#[cfg(target_os = "macos")]
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
#[cfg(target_os = "macos")]
use std::ptr;

pub type GlGetProcAddressFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
pub type GlGetIntegervFn = unsafe extern "C" fn(u32, *mut i32);

/// Keeps GL/EGL shared objects loaded and exposes proc resolution + `glGetIntegerv`.
pub struct GlDynLib {
    #[cfg(target_os = "linux")]
    _egl: Library,
    #[cfg(target_os = "linux")]
    _gl: Library,
    pub get_proc: GlGetProcAddressFn,
    pub gl_get_integerv: GlGetIntegervFn,
}

impl GlDynLib {
    pub fn load() -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            let _egl = unsafe { Library::new("libEGL.so.1") }.map_err(|e| e.to_string())?;
            let _gl = unsafe { Library::new("libGL.so.1") }.map_err(|e| e.to_string())?;
            // Copy fn pointers out of [Symbol] before moving [Library] (Symbol borrows Library).
            let get_proc = *unsafe { _egl.get(b"eglGetProcAddress\0") }.map_err(|e| e.to_string())?;
            let gl_get_integerv =
                *unsafe { _gl.get(b"glGetIntegerv\0") }.map_err(|e| e.to_string())?;
            Ok(Self {
                _egl,
                _gl,
                get_proc,
                gl_get_integerv,
            })
        }
        #[cfg(target_os = "macos")]
        {
            let gl_get_integerv = unsafe { macos_gl_get_integerv() }?;
            Ok(Self {
                get_proc: macos_gl_get_proc_address,
                gl_get_integerv,
            })
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Rhino Player builds on Linux and macOS only");

#[cfg(target_os = "macos")]
unsafe fn macos_gl_get_integerv() -> Result<GlGetIntegervFn, String> {
    for name in [b"glGetIntegerv\0".as_ptr(), b"_glGetIntegerv\0".as_ptr()] {
        let p = libc::dlsym(libc::RTLD_DEFAULT, name.cast());
        if !p.is_null() {
            return Ok(std::mem::transmute::<*mut c_void, GlGetIntegervFn>(p));
        }
    }
    Err(
        "glGetIntegerv not found (GTK 4 GL backend missing?). Install GTK with OpenGL support."
            .into(),
    )
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_gl_get_proc_address(name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    let base = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let try_sym = |n: &str| {
        CString::new(n).ok().and_then(|c| {
            let p = libc::dlsym(libc::RTLD_DEFAULT, c.as_ptr().cast());
            if p.is_null() {
                None
            } else {
                Some(p)
            }
        })
    };
    try_sym(base)
        .or_else(|| try_sym(&format!("_{base}")))
        .or_else(|| try_sym(&format!("{base}_OES")))
        .or_else(|| try_sym(&format!("{base}_ARB")))
        .or_else(|| try_sym(&format!("{base}_EXT")))
        .unwrap_or(ptr::null_mut())
}

/// libmpv `ao` default for the host (Pulse on Linux, CoreAudio on macOS).
pub fn mpv_default_audio_output() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "coreaudio"
    }
    #[cfg(target_os = "linux")]
    {
        "pulse"
    }
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    {
        "auto"
    }
    #[cfg(not(unix))]
    {
        "auto"
    }
}
