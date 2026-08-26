//! Raw libmpv `mpv_render_context` creation plus the update / draw callbacks wired onto
//! it. Split from `macos_video_bundle.rs` so each module stays small.

use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::Arc;

use libmpv2::Mpv;
use libmpv2_sys::{
    mpv_opengl_fbo, mpv_opengl_init_params, mpv_render_context, mpv_render_context_create,
    mpv_render_context_render, mpv_render_context_report_swap,
    mpv_render_context_set_update_callback, mpv_render_param,
    mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE as PARAM_API_TYPE,
    mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y as PARAM_FLIP_Y,
    mpv_render_param_type_MPV_RENDER_PARAM_INVALID as PARAM_INVALID,
    mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO as PARAM_OPENGL_FBO,
    mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS as PARAM_OPENGL_INIT_PARAMS,
};

use super::super::macos_video_cgl::GlSymbolLoader;
use super::super::macos_video_displaylink::DriverStateHandle;
use super::NativeVideoSurface;

/// Cheap `Send` payload handed to mpv's update callback. Holds an [`Arc`] to the
/// displayLink handle so flipping the pending bit is just an atomic store.
pub(super) struct UpdateCtx {
    pub(super) handle: Arc<DriverStateHandle>,
}

/// Input-only `PARAM_FLIP_Y` payload — libmpv reads this int, it never writes it.
static FLIP_Y_ONE: c_int = 1;

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

pub(super) fn create_render_context(
    mpv: &mut Mpv,
    gl_loader: &Arc<GlSymbolLoader>,
) -> Result<*mut mpv_render_context, String> {
    let api_type = c"opengl".as_ptr() as *mut c_void;
    let mut init = mpv_opengl_init_params {
        get_proc_address: Some(gl_get_proc_address),
        get_proc_address_ctx: Arc::as_ptr(gl_loader) as *mut c_void,
    };
    let mut params: [mpv_render_param; 3] = [
        mpv_render_param {
            type_: PARAM_API_TYPE,
            data: api_type,
        },
        mpv_render_param {
            type_: PARAM_OPENGL_INIT_PARAMS,
            data: &mut init as *mut _ as *mut c_void,
        },
        mpv_render_param {
            type_: PARAM_INVALID,
            data: ptr::null_mut(),
        },
    ];
    let mut rctx: *mut mpv_render_context = ptr::null_mut();
    let err =
        unsafe { mpv_render_context_create(&mut rctx, mpv.ctx.as_ptr(), params.as_mut_ptr()) };
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

pub(super) fn wire_update_callback(rctx: *mut mpv_render_context, ctx: &UpdateCtx) {
    let ctx_ptr = ctx as *const UpdateCtx as *mut c_void;
    unsafe {
        mpv_render_context_set_update_callback(rctx, Some(on_mpv_update), ctx_ptr);
    }
}

pub(super) fn wire_draw_callback(rctx: *mut mpv_render_context, surface: &NativeVideoSurface) {
    let render_ctx_addr = rctx as usize;
    let redraw = surface.redraw_handle();
    const GL_RGBA8: c_int = 0x8058;
    surface.set_draw_callback(Some(Box::new(move |fbo, w, h| {
        if w <= 0 || h <= 0 {
            return;
        }
        if redraw.vf_teardown_suppress_active() {
            return;
        }
        let mut fbo_data = mpv_opengl_fbo {
            fbo,
            w,
            h,
            internal_format: GL_RGBA8,
        };
        let mut params: [mpv_render_param; 3] = [
            mpv_render_param {
                type_: PARAM_OPENGL_FBO,
                data: &mut fbo_data as *mut _ as *mut c_void,
            },
            mpv_render_param {
                type_: PARAM_FLIP_Y,
                data: &FLIP_Y_ONE as *const c_int as *mut c_void,
            },
            mpv_render_param {
                type_: PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];
        unsafe {
            let ctx = render_ctx_addr as *mut mpv_render_context;
            mpv_render_context_render(ctx, params.as_mut_ptr());
            // **`display-resample`** needs swap timing. Linux gates plain **`audio`** playback off; macOS keeps swaps for **`CVDisplayLink`**.
            if crate::video_pref::smooth_vf_timing_report_active() {
                mpv_render_context_report_swap(ctx);
            }
        }
    })));
}
