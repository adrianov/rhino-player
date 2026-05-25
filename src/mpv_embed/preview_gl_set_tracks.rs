/// Auxiliary thumbnail player: video-only [libmpv] with [vo=libmpv], isolated from user playback
/// settings, tracks, scripts, watch-later, and resume state.
pub struct MpvPreviewGl {
    _gl: GlDynLib,
    pub mpv: Mpv,
    render: RenderContext,
    gl_ptr: usize,
}

impl MpvPreviewGl {
    /// Call from [gtk::GLArea::connect_realize] with a current context ([make_current]).
    pub fn new(gl_area: &gtk::GLArea) -> Result<Self, String> {
        let gl_libs = GlDynLib::load()?;
        let egl_state = EglState {
            get: gl_libs.get_proc,
        };

        let mut mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("ao", "null")?;
            init.set_option("osc", "no")?;
            init.set_option("load-scripts", false)?;
            init.set_option("config", "no")?;
            init.set_option("ytdl", false)?;
            init.set_option("pause", true)?;
            // Thumbnail seeks near EOF must not unload the clip (default EOF → idle/black).
            let _ = init.set_option("keep-open", "always");
            let _ = init.set_option("autoload-files", "no");
            let _ = init.set_option("audio-file-auto", "no");
            let _ = init.set_option("sub-auto", "no");
            let _ = init.set_option("aid", "no");
            let _ = init.set_option("sid", "no");
            let _ = init.set_option("secondary-sid", "no");
            let _ = init.set_option("resume-playback", "no");
            let _ = init.set_option("save-position-on-quit", "no");
            let _ = init.set_option("hwdec", "no");
            let _ = init.set_option("terminal", false);
            let _ = init.set_option("msg-level", "all=no");
            let _ = init.set_option("vd-lavc-threads", 2i64);
            let _ = init.set_option("vd-lavc-fast", true);
            let _ = init.set_option("vd-lavc-skiploopfilter", "all");
            let _ = init.set_option("vd-lavc-skipidct", "nonkey");
            let _ = init.set_option("vd-lavc-skipframe", "nonkey");
            let _ = init.set_option("vd-lavc-software-fallback", 1i64);
            let _ = init.set_option("sws-scaler", "fast-bilinear");
            let _ = init.set_option("demuxer-readahead-secs", 0.0f64);
            let _ = init.set_option("demuxer-max-bytes", "128KiB");
            let _ = init.set_option("hr-seek", false);
            let _ = init.set_option("gpu-dumb-mode", true);
            let _ = init.set_option("load-osd-console", "no");
            let _ = init.set_option("load-stats-overlay", "no");
            let _ = init.set_option("load-auto-profiles", "no");
            let _ = init.set_option("really-quiet", "yes");
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;

        let _ = mpv.set_property("pause", true);
        set_preview_tracks(&mpv);
        let _ = mpv.disable_deprecated_events();

        let params: Vec<RenderParam<EglState>> = vec![
            RenderParam::ApiType(RenderParamApiType::OpenGl),
            RenderParam::InitParams(OpenGLInitParams {
                get_proc_address: egl_proc,
                ctx: egl_state,
            }),
        ];

        let mut render = RenderContext::new(unsafe { mpv.ctx.as_mut() }, params)
            .map_err(|e| format!("render context: {e:?}"))?;

        let gl_ptr = gl_area.upcast_ref::<glib::Object>().as_ptr() as usize;
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

        Ok(Self {
            _gl: gl_libs,
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
        unsafe { (self._gl.gl_get_integerv)(GL_FRAMEBUFFER_BINDING, &mut fbo) };
        if self.render.render::<EglState>(fbo, w, h, true).is_ok() {
            self.render.report_swap();
        } else {
            crate::preview_debug::warn(format!("render failed {w}x{h} fbo={fbo}"));
        }
    }

    /// Black out the GLArea (call with a current preview context).
    pub fn clear_framebuffer(&self, area: &gtk::GLArea) {
        if area.upcast_ref::<glib::Object>().as_ptr() as usize != self.gl_ptr {
            return;
        }
        if !area.is_realized() {
            return;
        }
        area.make_current();
        type GlClearColor = unsafe extern "C" fn(f32, f32, f32, f32);
        type GlClear = unsafe extern "C" fn(u32);
        const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
        unsafe {
            let get = self._gl.get_proc;
            let cc_name = std::ffi::CString::new("glClearColor").expect("cstring");
            let cl_name = std::ffi::CString::new("glClear").expect("cstring");
            let cc_ptr = get(cc_name.as_ptr());
            let cl_ptr = get(cl_name.as_ptr());
            if cc_ptr.is_null() || cl_ptr.is_null() {
                return;
            }
            let cc: GlClearColor = std::mem::transmute(cc_ptr);
            let cl: GlClear = std::mem::transmute(cl_ptr);
            cc(0.0, 0.0, 0.0, 1.0);
            cl(GL_COLOR_BUFFER_BIT);
        }
    }

    /// Tear down render context + mpv without running [`Mpv`]'s [`Drop`] (aborts with `vo=libmpv`).
    pub fn dispose(self, gl_area: &gtk::GLArea) {
        gl_area.make_current();
        let Self {
            _gl: _,
            mut mpv,
            render,
            gl_ptr: _,
        } = self;
        mpv.set_wakeup_callback(|| {});
        drop(render);
        unsafe {
            libmpv2_sys::mpv_terminate_destroy(mpv.ctx.as_ptr());
        }
        std::mem::forget(mpv);
    }
}

pub fn set_preview_tracks(mpv: &Mpv) {
    let _ = mpv.set_property("aid", "no");
    let _ = mpv.set_property("sid", "no");
    let _ = mpv.set_property("secondary-sid", "no");
    let _ = mpv.set_property("pause", true);
}
