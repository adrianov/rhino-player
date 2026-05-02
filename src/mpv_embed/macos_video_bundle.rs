//! macOS native render setup for [`super::MpvBundle`]. Owns the [`NativeVideoSurface`],
//! creates `mpv_render_context` directly via libmpv2-sys (so we keep the raw pointer to
//! pass to the layer's draw callback), and wires update / draw callbacks.

#![allow(deprecated)]

use std::ffi::{CStr, c_void};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::Arc;

use glib::object::IsA;
use gtk::prelude::WidgetExt;
use libmpv2::Mpv;
use libmpv2_sys::{
    mpv_opengl_fbo, mpv_opengl_init_params, mpv_render_context, mpv_render_context_create,
    mpv_render_context_free, mpv_render_context_render, mpv_render_context_set_update_callback,
    mpv_render_param,
    mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE as PARAM_API_TYPE,
    mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y as PARAM_FLIP_Y,
    mpv_render_param_type_MPV_RENDER_PARAM_INVALID as PARAM_INVALID,
    mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO as PARAM_OPENGL_FBO,
    mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS as PARAM_OPENGL_INIT_PARAMS,
};

use super::macos_video_attach::{self, NativeVideoSurface};
use super::macos_video_cgl::GlSymbolLoader;
use super::macos_video_displaylink::DriverStateHandle;

const GL_COLOR_BUFFER_BIT: c_int = 0x4000;

/// Clear the **GLArea**'s default framebuffer (the one GTK manages, not our native
/// layer) with `(0, 0, 0, 0)` so the GLArea's region in gdk-macos's compositing tree
/// has alpha=0. Combined with `insertSublayer:atIndex:0` for the native video layer
/// this lets the video show through the GLArea while the GTK chrome (which gdk paints
/// into the same sublayer) stays opaque on top.
///
/// Must run with the GLArea's GL context current (call from inside `connect_render`).
/// The symbols are looked up once via `dlsym(RTLD_DEFAULT)` and cached.
pub fn clear_glarea_transparent() {
    use std::sync::OnceLock;
    type ClearColorFn = unsafe extern "C" fn(f32, f32, f32, f32);
    type ClearFn = unsafe extern "C" fn(c_int);
    static SYMS: OnceLock<Option<(ClearColorFn, ClearFn)>> = OnceLock::new();
    let syms = SYMS.get_or_init(|| {
        let loader = GlSymbolLoader::open().ok()?;
        let cc = loader.lookup("glClearColor");
        let cl = loader.lookup("glClear");
        if cc.is_null() || cl.is_null() {
            return None;
        }
        unsafe {
            Some((
                std::mem::transmute::<*mut c_void, ClearColorFn>(cc),
                std::mem::transmute::<*mut c_void, ClearFn>(cl),
            ))
        }
    });
    if let Some((clear_color, clear)) = syms {
        unsafe {
            clear_color(0.0, 0.0, 0.0, 0.0);
            clear(GL_COLOR_BUFFER_BIT);
        }
    }
}

/// macOS render plumbing tied to one [`Mpv`] instance. Drop order matters — see [`Drop`].
pub struct MacosRender {
    /// CAOpenGLLayer + NSView + CVDisplayLink driver. Must outlive the render context
    /// (the layer's draw callback dereferences `render_ctx`, and the displayLink keeps
    /// firing into the layer until we drop it).
    surface: Box<NativeVideoSurface>,
    /// Raw mpv render context. Owned here; freed in [`Drop`].
    render_ctx: *mut mpv_render_context,
    /// Boxed so the raw pointer we hand to `mpv_render_context_set_update_callback`
    /// stays valid even if the surrounding `MpvBundle` is moved.
    update_ctx: Box<UpdateCtx>,
    /// Keeps the OpenGL.framework dlopen handle alive for `get_proc_address`.
    _gl_loader: Arc<GlSymbolLoader>,
}

/// Cheap `Send` payload handed to mpv's update callback. Holds an [`Arc`] to the
/// displayLink handle so flipping the pending bit is just an atomic store.
struct UpdateCtx {
    handle: Arc<DriverStateHandle>,
}

unsafe impl Send for MacosRender {}

impl MacosRender {
    /// Create the surface, attach it to the GTK window's NSWindow, build the mpv render
    /// context against the surface's CGL context, wire callbacks.
    pub fn install(mpv: &mut Mpv, sizer: &gtk::GLArea) -> Result<Self, String> {
        let surface = Box::new(macos_video_attach::install(sizer)?);
        let gl_loader = surface.gl_loader();

        let render_ctx = create_render_context(mpv, &gl_loader)?;
        let update_ctx = Box::new(UpdateCtx { handle: surface.redraw_handle() });
        wire_update_callback(render_ctx, &update_ctx);
        wire_draw_callback(render_ctx, &surface);

        // GLArea must stay visible for `compute_point` (the size-tracking helper) to work,
        // but its OpenGL output is never seen — the NSView covers it.
        sizer.set_visible(true);

        Ok(Self {
            surface,
            render_ctx,
            update_ctx,
            _gl_loader: gl_loader,
        })
    }

    /// When `widget` is visible, hide the native video layer so a GTK overlay (e.g. the
    /// recent-files grid) can paint through. Drives both `notify::visible` and the
    /// per-frame tick comparison inside the surface.
    pub fn watch_overlay<W: IsA<gtk::Widget>>(&self, widget: &W) {
        self.surface.watch_overlay(widget);
    }
}

impl Drop for MacosRender {
    fn drop(&mut self) {
        // Order matters: stop the displayLink before freeing the render context (the
        // displayLink callback dereferences the layer, which dereferences the draw
        // closure, which holds the render context pointer). Then drop the update
        // callback so mpv stops poking the (about-to-die) `update_ctx`. Then free.
        self.surface.detach();
        unsafe {
            mpv_render_context_set_update_callback(self.render_ctx, None, ptr::null_mut());
            mpv_render_context_free(self.render_ctx);
        }
        let _ = &self.update_ctx;
    }
}

unsafe extern "C" fn gl_get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    if ctx.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    let loader = unsafe { &*(ctx as *const GlSymbolLoader) };
    let cstr = unsafe { CStr::from_ptr(name) };
    match cstr.to_str() {
        Ok(s) => loader.lookup(s),
        Err(_) => ptr::null_mut(),
    }
}

fn create_render_context(
    mpv: &mut Mpv,
    gl_loader: &Arc<GlSymbolLoader>,
) -> Result<*mut mpv_render_context, String> {
    let api_type = c"opengl".as_ptr() as *mut c_void;
    let mut init = mpv_opengl_init_params {
        get_proc_address: Some(gl_get_proc_address),
        get_proc_address_ctx: Arc::as_ptr(gl_loader) as *mut c_void,
    };
    let mut params: [mpv_render_param; 3] = [
        mpv_render_param { type_: PARAM_API_TYPE, data: api_type },
        mpv_render_param { type_: PARAM_OPENGL_INIT_PARAMS, data: &mut init as *mut _ as *mut c_void },
        mpv_render_param { type_: PARAM_INVALID, data: ptr::null_mut() },
    ];
    let mut rctx: *mut mpv_render_context = ptr::null_mut();
    let err = unsafe { mpv_render_context_create(&mut rctx, mpv.ctx.as_ptr(), params.as_mut_ptr()) };
    if err < 0 || rctx.is_null() {
        return Err(format!("mpv_render_context_create failed: {err}"));
    }
    Ok(rctx)
}

unsafe extern "C" fn on_mpv_update(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let cx = unsafe { &*(ctx as *const UpdateCtx) };
    cx.handle.mark_pending();
}

fn wire_update_callback(rctx: *mut mpv_render_context, ctx: &UpdateCtx) {
    let ctx_ptr = ctx as *const UpdateCtx as *mut c_void;
    unsafe {
        mpv_render_context_set_update_callback(rctx, Some(on_mpv_update), ctx_ptr);
    }
}

fn wire_draw_callback(rctx: *mut mpv_render_context, surface: &NativeVideoSurface) {
    let render_ctx_addr = rctx as usize;
    const GL_RGBA8: c_int = 0x8058;
    surface.set_draw_callback(Some(Box::new(move |fbo, w, h| {
        if w <= 0 || h <= 0 {
            return;
        }
        let mut fbo_data = mpv_opengl_fbo {
            fbo,
            w,
            h,
            internal_format: GL_RGBA8,
        };
        let mut flip: c_int = 1;
        let mut params: [mpv_render_param; 3] = [
            mpv_render_param { type_: PARAM_OPENGL_FBO, data: &mut fbo_data as *mut _ as *mut c_void },
            mpv_render_param { type_: PARAM_FLIP_Y, data: &mut flip as *mut _ as *mut c_void },
            mpv_render_param { type_: PARAM_INVALID, data: ptr::null_mut() },
        ];
        unsafe {
            mpv_render_context_render(
                render_ctx_addr as *mut mpv_render_context,
                params.as_mut_ptr(),
            );
        }
    })));
}
