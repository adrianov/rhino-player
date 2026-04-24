//! libmpv OpenGL output in a [`gtk::GLArea`]. See `docs/features/03-mpv-embedding.md` and `docs/product-and-use-cases.md`.

use glib::prelude::Cast;
use glib::translate::from_glib_borrow;
use gtk::prelude::*;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use libloading::{Library, Symbol};
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::ptr;

type EglGetProcAddress = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type GlGetIntegerv = unsafe extern "C" fn(u32, *mut i32);

const GL_FRAMEBUFFER_BINDING: u32 = 0x8ca6;

#[derive(Copy, Clone)]
struct EglState {
    get: EglGetProcAddress,
}

fn egl_proc(s: &EglState, name: &str) -> *mut c_void {
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
        .unwrap_or(ptr::null_mut())
}

/// Owns [`libloading::Library`] for `libEGL` / `libGL` for the process lifetime.
pub struct MpvBundle {
    _egl: Library,
    _gl: Library,
    gl_get: GlGetIntegerv,
    pub mpv: Mpv,
    render: RenderContext,
    gl_ptr: usize,
}

impl MpvBundle {
    /// Call with a current GL context on `gl_area` (e.g. inside `GLArea::realize`).
    pub fn new(gl_area: &gtk::GLArea) -> Result<Self, String> {
        let _egl = unsafe { Library::new("libEGL.so.1") }.map_err(|e| e.to_string())?;
        let _gl = unsafe { Library::new("libGL.so.1") }.map_err(|e| e.to_string())?;

        let egl_get: Symbol<EglGetProcAddress> =
            unsafe { _egl.get(b"eglGetProcAddress\0") }.map_err(|e| e.to_string())?;
        let gl_get: Symbol<GlGetIntegerv> = unsafe { _gl.get(b"glGetIntegerv\0") }.map_err(|e| e.to_string())?;

        let egl_state = EglState { get: *egl_get };
        let gl_get = *gl_get;

        let mut mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("osc", "no")?;
            let _ = init.set_option("ao", "pulse");
            let _ = init.set_option("keep-open", "yes");
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;

        let params: Vec<RenderParam<EglState>> = vec![
            RenderParam::ApiType(RenderParamApiType::OpenGl),
            RenderParam::InitParams(OpenGLInitParams {
                get_proc_address: egl_proc,
                ctx: egl_state,
            }),
        ];

        let mut render = RenderContext::new(unsafe { mpv.ctx.as_mut() }, params.into_iter())
            .map_err(|e| format!("render context: {e:?}"))?;

        let gl_ptr = gl_area.upcast_ref::<glib::Object>().as_ptr() as usize;
        let mctx = glib::MainContext::default();
        render.set_update_callback(move || {
            let p = gl_ptr;
            mctx.clone().invoke(move || {
                let gl = unsafe {
                    from_glib_borrow::<*mut gtk::ffi::GtkGLArea, gtk::GLArea>(p as *mut gtk::ffi::GtkGLArea)
                };
                gl.queue_render();
            });
        });

        Ok(Self {
            _egl,
            _gl,
            gl_get,
            mpv,
            render,
            gl_ptr,
        })
    }

    pub fn draw(&self, area: &gtk::GLArea) {
        if area.upcast_ref::<glib::Object>().as_ptr() as usize != self.gl_ptr {
            return;
        }
        let scale = area.scale_factor();
        let w = area.width() * scale;
        let h = area.height() * scale;
        if w <= 0 || h <= 0 {
            return;
        }
        let mut fbo: i32 = 0;
        unsafe { (self.gl_get)(GL_FRAMEBUFFER_BINDING, &mut fbo) };
        let _ = self.render.render::<EglState>(fbo, w, h, true);
        self.render.report_swap();
    }
}
