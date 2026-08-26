//! macOS native render setup for [`super::MpvBundle`]. Owns the [`NativeVideoSurface`],
//! creates `mpv_render_context` directly via libmpv2-sys (so we keep the raw pointer to
//! pass to the layer's draw callback), and wires update / draw callbacks.

#![allow(deprecated)]

use std::os::raw::c_int;
use std::ptr;
use std::sync::Arc;

use glib::object::IsA;
use gtk::prelude::WidgetExt;
use libmpv2::Mpv;
use libmpv2_sys::{
    mpv_render_context, mpv_render_context_free, mpv_render_context_set_update_callback,
    mpv_render_context_update,
};

use super::macos_video_attach::{self, NativeVideoSurface};
use super::macos_video_cgl::GlSymbolLoader;

mod render_context;

use self::render_context::{create_render_context, wire_draw_callback, wire_update_callback};

const GL_COLOR_BUFFER_BIT: c_int = 0x4000;

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
    update_ctx: Box<render_context::UpdateCtx>,
    /// `OpenGL.framework` dlopen handle. Held for the render context's `get_proc_address`
    /// callback **and** reused by `clear_glarea_transparent` (no second dlopen).
    gl_loader: Arc<GlSymbolLoader>,
}

unsafe impl Send for MacosRender {}

impl MacosRender {
    /// Create the surface, attach it to the GTK window's NSWindow, build the mpv render
    /// context against the surface's CGL context, wire callbacks.
    pub fn install(mpv: &mut Mpv, sizer: &gtk::GLArea) -> Result<Self, String> {
        let surface = Box::new(macos_video_attach::install(sizer)?);
        let gl_loader = surface.gl_loader();

        let render_ctx = create_render_context(mpv, &gl_loader)?;
        let update_ctx = Box::new(render_context::UpdateCtx {
            handle: surface.redraw_handle(),
        });
        wire_update_callback(render_ctx, &update_ctx);
        wire_draw_callback(render_ctx, &surface);

        // GLArea must stay visible for `compute_point` (the size-tracking helper) to work,
        // but its OpenGL output is never seen — the NSView covers it.
        sizer.set_visible(true);

        Ok(Self {
            surface,
            render_ctx,
            update_ctx,
            gl_loader,
        })
    }

    /// When `widget` is visible, hide the native video layer so a GTK overlay (e.g. the
    /// recent-files grid) can paint through. Drives both `notify::visible` and the
    /// per-frame tick comparison inside the surface.
    pub fn watch_overlay<W: IsA<gtk::Widget>>(&self, widget: &W) {
        self.surface.watch_overlay(widget);
    }

    pub fn resync_layer_frame(&self) {
        self.surface.resync_layer_frame();
    }

    pub fn repin_below_gtk_compositing(&self) {
        self.surface.repin_below_gtk_compositing();
    }

    /// Serialize **`vf clr`** vs **`CVDisplayLink`** / **`mpv_render_context_render`** (Smooth **off**).
    pub(crate) fn with_vf_teardown<R>(&self, f: impl FnOnce() -> R) -> R {
        self.surface.pause_cv_display_link();
        let h = self.surface.redraw_handle();
        h.begin_vf_teardown();
        let out = f();
        h.end_vf_teardown();
        self.surface.resume_cv_display_link();
        out
    }

    pub(crate) fn mark_display_pending(&self) {
        self.surface.redraw_handle().mark_pending();
    }

    /// Wake **`vo=libmpv`** render state after **`vf`** changes (bitmask per mpv **`mpv_render_context_update`**).
    pub(crate) fn ping_render_context(&self) -> u64 {
        unsafe { mpv_render_context_update(self.render_ctx) }
    }

    /// Clear the GLArea's framebuffer to alpha=0 so gdk-macos's compositing produces
    /// transparent pixels in the GLArea region — the native CAOpenGLLayer **below** then
    /// shows through. Reuses the same `OpenGL.framework` handle that the render context
    /// holds, so we never `dlopen` it twice.
    pub fn clear_glarea_transparent(&self) {
        let Some((clear_color, clear)) = self.gl_loader.cached_clear_syms() else {
            return;
        };
        unsafe {
            clear_color(0.0, 0.0, 0.0, 0.0);
            clear(GL_COLOR_BUFFER_BIT);
        }
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
