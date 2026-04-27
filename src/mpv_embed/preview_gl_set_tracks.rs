/// Auxiliary thumbnail player: video-only [libmpv] with [vo=libmpv], isolated from user playback
/// settings, tracks, scripts, watch-later, and resume state.
pub struct MpvPreviewGl {
    _egl: Library,
    _gl: Library,
    gl_get: GlGetIntegerv,
    pub mpv: Mpv,
    render: RenderContext,
    gl_ptr: usize,
}

impl MpvPreviewGl {
    /// Call from [gtk::GLArea::connect_realize] with a current context ([make_current]).
    pub fn new(gl_area: &gtk::GLArea) -> Result<Self, String> {
        let _egl = unsafe { Library::new("libEGL.so.1") }.map_err(|e| e.to_string())?;
        let _gl = unsafe { Library::new("libGL.so.1") }.map_err(|e| e.to_string())?;

        let egl_get: Symbol<EglGetProcAddress> =
            unsafe { _egl.get(b"eglGetProcAddress\0") }.map_err(|e| e.to_string())?;
        let gl_get: Symbol<GlGetIntegerv> =
            unsafe { _gl.get(b"glGetIntegerv\0") }.map_err(|e| e.to_string())?;

        let egl_state = EglState { get: *egl_get };
        let gl_get = *gl_get;

        let mut mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("ao", "null")?;
            init.set_option("osc", "no")?;
            init.set_option("load-scripts", false)?;
            init.set_option("config", "no")?;
            init.set_option("ytdl", false)?;
            init.set_option("pause", true)?;
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
        if self.render.render::<EglState>(fbo, w, h, true).is_ok() {
            self.render.report_swap();
        }
    }
}

pub fn set_preview_tracks(mpv: &Mpv) {
    let _ = mpv.set_property("aid", "no");
    let _ = mpv.set_property("sid", "no");
    let _ = mpv.set_property("secondary-sid", "no");
    let _ = mpv.set_property("pause", true);
}
