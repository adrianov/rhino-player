
use glib::prelude::Cast;
use glib::translate::from_glib_borrow;
use gtk::prelude::*;
pub use libmpv2::events::{Event, PropertyData};
pub use libmpv2::Format;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use std::os::raw::c_void;
use std::path::Path;
use std::ptr;

use crate::db;
use crate::db::VideoPrefs;
use crate::media_probe;
use crate::video_pref::apply_mpv_video_init;
use gl_platform::GlDynLib;

const GL_FRAMEBUFFER_BINDING: u32 = 0x8ca6;

#[derive(Copy, Clone)]
struct EglState {
    get: gl_platform::GlGetProcAddressFn,
}

fn egl_proc(s: &EglState, name: &str) -> *mut c_void {
    let try_name = |n: &str| {
        std::ffi::CString::new(n).ok().and_then(|c| {
            let p = unsafe { (s.get)(c.as_ptr()) };
            if p.is_null() {
                None
            } else {
                Some(p)
            }
        })
    };
    try_name(name)
        .or_else(|| try_name(&format!("{name}_OES")))
        .or_else(|| try_name(&format!("{name}_ARB")))
        .or_else(|| try_name(&format!("{name}_EXT")))
        .unwrap_or(ptr::null_mut())
}

/// Owns loaded GL/EGL (Linux) or relies on GTK’s GL (macOS); see [`GlDynLib`].
pub struct MpvBundle {
    _gl: GlDynLib,
    pub mpv: Mpv,
    render: RenderContext,
    gl_ptr: usize,
    /// Resume time (seconds) for the next `FileLoaded`. Set by [load_file_path] from `db::resume_pos`,
    /// applied + cleared by [apply_pending_resume] after the file is loaded.
    pending_resume: std::cell::Cell<Option<f64>>,
    /// Last **GLArea** drawable height in physical pixels (`scale_factor × widget height`). Combined
    /// with mpv **`height`** / **`dheight`** for MVTools CPU tiering (`video_pref::smooth_motion_cost_height`).
    last_draw_h: std::cell::Cell<i32>,
}

impl MpvBundle {
    /// Call with a current GL context on `gl_area` (e.g. inside `GLArea::realize`).
    /// [VideoPrefs] (optional VapourSynth 60 fps `vf`) from SQLite; see [apply_mpv_video].
    /// The `bool` is `true` when **Smooth Video (~60 FPS at 1.0×)** was auto-disabled (VapourSynth `vf` rejected); sync UI.
    pub fn new(gl_area: &gtk::GLArea, video: &mut VideoPrefs) -> Result<(Self, bool), String> {
        let gl_libs = GlDynLib::load()?;
        let egl_state = EglState {
            get: gl_libs.get_proc,
        };

        let mut mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("osc", "no")?;
            // 0 = auto: libavcodec can use multiple CPU threads for software decode
            // (independent of heavy single-threaded sections in some filters / MVTools).
            let _ = init.set_option("vd-lavc-threads", "0");
            let _ = init.set_option("ao", gl_platform::mpv_default_audio_output());
            let _ = init.set_option("keep-open", "yes");
            // Resume position is owned by SQLite (`db::resume_pos` → `loadfile … start=`); mpv's
            // watch_later mechanism is disabled to avoid double-bookkeeping and to keep `speed` /
            // `pause` from leaking across sessions.
            let _ = init.set_option("save-position-on-quit", "no");
            let _ = init.set_option("resume-playback", "no");
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;

        let auto_off = apply_mpv_video_init(&mpv, video).smooth_auto_off;
        // Thumbnails: prefer JPEG (fast); PNG path uses minimum compression.
        let _ = mpv.set_property("screenshot-format", "jpeg");
        let _ = mpv.set_property("screenshot-jpeg-quality", 90i64);
        let _ = mpv.set_property("screenshot-png-compression", 0i64);

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

        Ok((
            Self {
                _gl: gl_libs,
                mpv,
                render,
                gl_ptr,
                pending_resume: std::cell::Cell::new(None),
                last_draw_h: std::cell::Cell::new(0),
            },
            auto_off,
        ))
    }

    pub(crate) fn smooth_draw_height_px(&self) -> i32 {
        self.last_draw_h.get()
    }

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
        self.last_draw_h.set(h);
        let mut fbo: i32 = 0;
        unsafe { (self._gl.gl_get_integerv)(GL_FRAMEBUFFER_BINDING, &mut fbo) };
        self.render.render::<EglState>(fbo, w, h, true).is_ok()
    }

    pub fn draw(&self, area: &gtk::GLArea) {
        let _ = self.draw_impl(area);
    }

    /// Final paint before dropping [`MpvBundle`]: render, swap report on success, then render-context update.
    /// Call only with GTK GL current on `area` (e.g. inside `GLArea::render`). Needed so libmpv can tear
    /// down the VO before `mpv_render_context_free`; skipping this triggers aborts on macOS GTK.
    pub fn teardown_gl_paint(&self, area: &gtk::GLArea) {
        if self.draw_impl(area) {
            self.render.report_swap();
        }
        let _ = self.render.update();
    }

    /// End playback; call after the SQLite snapshot. Safe to skip before process exit.
    pub fn stop_playback(&self) {
        let _ = self.mpv.command("stop", &[]);
    }

    /// Persist `duration` + `time-pos` (and clear them if at natural end) for the open local file.
    /// Single source of truth for resume — replaces the old mpv `watch_later` sidecar dance.
    pub fn save_playback_state(&self) {
        if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
            if media_probe::is_natural_end(&self.mpv) {
                media_probe::clear_resume_for_path(&p);
                return;
            }
        }
        media_probe::record_playback_for_current(&self.mpv);
    }

    /// Save SQLite resume snapshot, then stop playback. Used at process quit.
    pub fn commit_quit(&self) {
        self.save_playback_state();
        self.stop_playback();
    }

    /// Save SQLite resume snapshot before leaving the open file (e.g. **Back to Browse**).
    pub fn snapshot_outgoing_before_leave(&self) {
        self.save_playback_state();
    }

    /// Subscribe to mpv property changes. Each tuple is `(reply_id, name, format)`.
    /// `reply_id` is echoed back on the [Event::PropertyChange] so handlers can dispatch quickly.
    pub fn observe_props(&self, props: &[(u64, &str, Format)]) -> Result<(), String> {
        for (id, name, fmt) in props {
            self.mpv
                .observe_property(name, *fmt, *id)
                .map_err(|e| format!("observe_property {name}: {e:?}"))?;
        }
        Ok(())
    }

    /// Wakeup-driven mpv event drain. The closure runs **on the GTK main thread** whenever
    /// libmpv has new events; the caller drains them with [drain_events]. The mpv wakeup
    /// callback is invoked from arbitrary mpv threads, so the closure is parked in a
    /// thread-local registered on the main thread, and a `Send` shim hops back over
    /// `MainContext::invoke`. See `events-over-polling.mdc`: do not call other mpv APIs
    /// from the wakeup callback itself.
    pub fn install_event_drain<F: Fn() + 'static>(&mut self, on_main: F) {
        thread_local! {
            static DRAIN: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
        }
        fn call_drain() {
            DRAIN.with(|s| { if let Some(f) = s.borrow().as_ref() { f(); } });
        }
        DRAIN.with(|s| *s.borrow_mut() = Some(Box::new(on_main)));
        let mctx = glib::MainContext::default();
        self.mpv.set_wakeup_callback(move || {
            mctx.clone().invoke(call_drain);
        });
    }

    /// Drain libmpv events until the queue is empty, dispatching each to `handler`.
    /// Call from the closure registered by [install_event_drain].
    pub fn drain_events(&mut self, mut handler: impl FnMut(Event<'_>)) {
        while let Some(ev) = self.mpv.wait_event(0.0) {
            match ev {
                Ok(e) => handler(e),
                Err(_) => continue,
            }
        }
    }

    /// End embedded playback for process quit without going through [`libmpv2::Mpv`]'s `Drop`, which
    /// invokes `mpv_destroy` and aborted with GTK `vo=libmpv` on macOS (`mp_clients_destroy`).
    ///
    /// Run [`teardown_gl_paint`] with `gl_area` current earlier in the teardown chain; `dispose_for_quit`
    /// calls [`gtk::prelude::GLAreaExt::make_current`] again before freeing the render context and calling
    /// `mpv_terminate_destroy`.
    pub fn dispose_for_quit(self, gl_area: &gtk::GLArea) {
        gl_area.make_current();
        let Self {
            _gl,
            mut mpv,
            render,
            ..
        } = self;
        mpv.set_wakeup_callback(|| {});
        drop(render);
        unsafe {
            libmpv2_sys::mpv_terminate_destroy(mpv.ctx.as_ptr());
        }
        std::mem::forget(mpv);
    }

    /// Save outgoing resume to SQLite, then `loadfile` the new path. The new file's resume position
    /// (if any in SQLite) is stashed in [pending_resume]; [apply_pending_resume] consumes it after
    /// `FileLoaded`. We do **not** pass `start=` as a loadfile option — older mpv (≤ 0.35) treats
    /// the third positional argument as `<index>` and rejects the whole command.
    /// When [clear_outgoing_resume] is true, the outgoing file reached the end: drop its DB resume.
    pub fn load_file_path(&self, path: &Path, clear_outgoing_resume: bool) -> Result<(), String> {
        if clear_outgoing_resume {
            if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
                media_probe::clear_resume_for_path(&p);
            }
        } else {
            media_probe::record_playback_for_current(&self.mpv);
        }
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
        self.pending_resume.set(db::resume_pos(&canonical));
        self.mpv
            .command("loadfile", &[s, "replace"])
            .map_err(|e| format!("{e:?}"))
    }

    /// Apply the resume stashed by the most recent [load_file_path]. Idempotent and a no-op when
    /// nothing is pending. Call once per `FileLoaded` from the transport-event drain.
    /// Uses **`absolute+exact`** so the demuxer lands on the saved time (keyframe-only seeks can
    /// sit at 0s briefly on load and fight the continue grid).
    pub fn apply_pending_resume(&self) {
        let Some(t) = self.pending_resume.replace(None) else {
            return;
        };
        let _ = crate::video_pref::unload_smooth_on_pause(&self.mpv);
        let s = format!("{t:.4}");
        let _ = self.mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    }
}
