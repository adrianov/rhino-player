// Cross-platform GL helpers for libmpv `OpenGLInitParams`. On Linux they back the main
// render context; on macOS only the seek-bar preview (`MpvPreviewGl`) still uses the
// GLArea path through them. Included from `mpv_embed.rs`.

const GL_FRAMEBUFFER_BINDING: u32 = 0x8ca6;

#[derive(Copy, Clone)]
struct EglState {
    get: gl_platform::GlGetProcAddressFn,
}

fn egl_try(get: gl_platform::GlGetProcAddressFn, n: &str) -> Option<*mut std::os::raw::c_void> {
    std::ffi::CString::new(n).ok().and_then(|c| {
        let p = unsafe { (get)(c.as_ptr()) };
        (!p.is_null()).then_some(p)
    })
}

fn egl_proc(s: &EglState, name: &str) -> *mut std::os::raw::c_void {
    egl_try(s.get, name)
        .or_else(|| egl_try(s.get, &format!("{name}_OES")))
        .or_else(|| egl_try(s.get, &format!("{name}_ARB")))
        .or_else(|| egl_try(s.get, &format!("{name}_EXT")))
        .unwrap_or(std::ptr::null_mut())
}
