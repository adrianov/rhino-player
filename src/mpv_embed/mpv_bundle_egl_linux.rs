// Linux EGL/GLArea render pipeline (`impl MpvBundle` extension). Included at module level
// so `main_bundle_egl_render.rs` keeps only platform-shaped construction; relies on
// `EglState` / `egl_proc` / `GL_FRAMEBUFFER_BINDING` from `linux_egl_helpers.rs`.

impl MpvBundle {
    /// Build the mpv render context against the process-loaded EGL library.
    #[cfg(not(target_os = "macos"))]
    fn new_egl_render_context(mpv: &mut Mpv, gl_libs: &GlDynLib) -> Result<RenderContext, String> {
        let egl_state = EglState {
            get: gl_libs.get_proc,
        };

        let params: Vec<RenderParam<EglState>> = vec![
            RenderParam::ApiType(RenderParamApiType::OpenGl),
            RenderParam::InitParams(OpenGLInitParams {
                get_proc_address: egl_proc,
                ctx: egl_state,
            }),
        ];

        RenderContext::new(unsafe { mpv.ctx.as_mut() }, params)
            .map_err(|e| format!("render context: {e:?}"))
    }

    /// Queue a GLArea redraw whenever mpv reports a new frame.
    #[cfg(not(target_os = "macos"))]
    fn install_queue_render_callback(render: &mut RenderContext, gl_ptr: usize) {
        let mctx = glib::MainContext::default();
        render.set_update_callback(move || {
            let p = gl_ptr;
            mctx.clone().invoke(move || {
                let gl = unsafe {
                    from_glib_borrow::<*mut gtk::ffi::GtkGLArea, gtk::GLArea>(
                        p as *mut gtk::ffi::GtkGLArea,
                    )
                };
                gl.queue_render();
            });
        });
    }

    #[cfg(not(target_os = "macos"))]
    fn finish_new(
        mut mpv: Mpv,
        gl_area: &gtk::GLArea,
        auto_off: bool,
    ) -> Result<(Self, bool), String> {
        let gl_libs = GlDynLib::load()?;
        let mut render = Self::new_egl_render_context(&mut mpv, &gl_libs)?;

        let gl_ptr = gl_area.upcast_ref::<glib::Object>().as_ptr() as usize;
        Self::install_queue_render_callback(&mut render, gl_ptr);
        Ok((
            mpv_bundle_self!(mpv, _gl: gl_libs, render, gl_ptr),
            auto_off,
        ))
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn linux_ping_render_context(&self) {
        let _ = self.render.update();
    }

    #[cfg(not(target_os = "macos"))]
    fn draw_impl(&self, area: &gtk::GLArea) -> bool {
        if area.upcast_ref::<glib::Object>().as_ptr() as usize != self.gl_ptr {
            return false;
        }
        let scale = area.scale_factor();
        let w = area.width() * scale;
        let h = area.height() * scale;
        if w <= 0 || h <= 0 {
            return false;
        }
        let mut fbo: i32 = 0;
        unsafe { (self._gl.gl_get_integerv)(GL_FRAMEBUFFER_BINDING, &mut fbo) };
        let ok = self.render.render::<EglState>(fbo, w, h, true).is_ok();
        if ok && crate::video_pref::smooth_vf_timing_report_active() {
            self.render.report_swap();
        }
        ok
    }

    /// Linux: render through the GLArea on the GTK frame clock. macOS: not used — the
    /// CAOpenGLLayer drives drawing from the displayLink, independent of GTK. The
    /// macOS render callback clears the GLArea with alpha=0 instead (see
    /// `macos_video_bundle::clear_glarea_transparent`).
    #[cfg(not(target_os = "macos"))]
    pub fn draw(&self, area: &gtk::GLArea) {
        let _ = self.draw_impl(area);
    }

    /// Final paint before dropping [`MpvBundle`]: render, swap report on success, then render-context update.
    /// Call only with GTK GL current on `area` (e.g. inside `GLArea::render`). Needed so libmpv can tear
    /// down the VO before `mpv_render_context_free`; skipping this triggers aborts on macOS GTK.
    pub fn teardown_gl_paint(&self, area: &gtk::GLArea) {
        #[cfg(not(target_os = "macos"))]
        {
            // `draw_impl` already calls `report_swap` when Smooth vf requests it; an unconditional
            // swap here confused VO timing after Smooth toggles / plain playback.
            let _ = self.draw_impl(area);
            let _ = self.render.update();
        }
        #[cfg(target_os = "macos")]
        let _ = area;
    }
}
