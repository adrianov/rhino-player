// Cross-platform GL helpers for libmpv `OpenGLInitParams`. On Linux they back the main
// render context; on macOS only the seek-bar preview (`MpvPreviewGl`) still uses the
// GLArea path through them. Included from `mpv_embed.rs`.

const GL_FRAMEBUFFER_BINDING: u32 = 0x8ca6;

#[derive(Copy, Clone)]
struct EglState {
    get: gl_platform::GlGetProcAddressFn,
}

fn egl_proc(s: &EglState, name: &str) -> *mut std::os::raw::c_void {
    let try_name = |n: &str| {
        std::ffi::CString::new(n).ok().and_then(|c| {
            let p = unsafe { (s.get)(c.as_ptr()) };
            if p.is_null() { None } else { Some(p) }
        })
    };
    try_name(name)
        .or_else(|| try_name(&format!("{name}_OES")))
        .or_else(|| try_name(&format!("{name}_ARB")))
        .or_else(|| try_name(&format!("{name}_EXT")))
        .unwrap_or(std::ptr::null_mut())
}
