
use glib::prelude::Cast;
use glib::translate::from_glib_borrow;
use gtk::prelude::*;
pub use libmpv2::events::{Event, PropertyData};
pub use libmpv2::Format;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use std::path::Path;

use crate::db;
use crate::db::VideoPrefs;
use crate::media_probe;
use crate::video_pref::apply_mpv_video_init;
use gl_platform::GlDynLib;

// EGL helper types (`EglState`, `egl_proc`, `GL_FRAMEBUFFER_BINDING`) live in
// `mpv_embed/linux_egl_helpers.rs` and are included into the same module.

/// Owns loaded GL/EGL (Linux) or a native [`CAOpenGLLayer`] surface (macOS).
pub struct MpvBundle {
    pub mpv: Mpv,
    /// Canonical path last set by the shell ([try_load], preload). SQLite ME budget + **`media`** keys use this
    /// **before** mpv **`path`**, which can lag after a switch.
    pub(crate) me_budget_shell_path: std::cell::RefCell<Option<std::path::PathBuf>>,
    /// Resume time (seconds) for the next `FileLoaded`. Set by [load_file_path] from `db::resume_pos`,
    /// applied + cleared by [apply_pending_resume] after the file is loaded.
    pending_resume: std::cell::Cell<Option<f64>>,
    /// Continue-grid warm hover: block SQLite `media` writes until the user opens for playback or closes.
    pub(crate) skip_media_persist: std::cell::Cell<bool>,
    /// Bumped on each warm `loadfile`; stale `FileLoaded` idles compare before resume/audio.
    pub(crate) warm_file_gen: std::cell::Cell<u32>,
    /// Pinned virtual DVD position until cross-chapter scrub resume is applied.
    pub(crate) dvd_hold_global: std::cell::Cell<Option<f64>>,
    /// Title-internal chapter `loadfile` from DVD EOF advance (keep vf, unpause after load).
    pub(crate) chapter_eof_load: std::cell::Cell<bool>,
    /// Cross-chapter unified-bar scrub: chapter-local [pending_resume]; ignore SQLite near-start.
    chapter_scrub_resume: std::cell::Cell<bool>,
    /// Hold `pause=yes` until cross-chapter resume seek lands (avoids playing from file start).
    chapter_scrub_hold_pause: std::cell::Cell<bool>,
    /// Unpause when hold ends when [load_chapter_seek] was called with `resume_playing=true`.
    chapter_scrub_unpause_after: std::cell::Cell<bool>,

    #[cfg(not(target_os = "macos"))]
    _gl: GlDynLib,
    #[cfg(not(target_os = "macos"))]
    render: RenderContext,
    #[cfg(not(target_os = "macos"))]
    gl_ptr: usize,

    /// macOS native render surface — owns the NSView, CAOpenGLLayer, dispatch queue, and
    /// the raw `mpv_render_context`. AppKit menu / popover tracking does not stall it.
    #[cfg(target_os = "macos")]
    macos: Option<crate::mpv_embed::macos_video_bundle::MacosRender>,
}

impl MpvBundle {
    /// Call with a current GL context on `gl_area` (Linux: inside `GLArea::realize`;
    /// macOS: any time the GtkWindow is realized — the GLArea here is used as a sizing
    /// placeholder, the render context binds to a native `CAOpenGLLayer` instead).
    ///
    /// [VideoPrefs] (optional VapourSynth 60 fps `vf`) from SQLite; see [apply_mpv_video].
    /// The `bool` is `true` when **Smooth Video (60 FPS)** was auto-disabled.
    pub fn new(gl_area: &gtk::GLArea, video: &mut VideoPrefs) -> Result<(Self, bool), String> {
        let mpv = Mpv::with_initializer(|init| {
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
            // Plain **`display-resample`** + **`report_swap`** via [apply_mpv_video_init] when Smooth off (Linux + macOS).
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;

        let auto_off = apply_mpv_video_init(&mpv, video).smooth_auto_off;
        // Thumbnails: prefer JPEG (fast); PNG path uses minimum compression.
        let _ = mpv.set_property("screenshot-format", "jpeg");
        let _ = mpv.set_property("screenshot-jpeg-quality", 90i64);
        let _ = mpv.set_property("screenshot-png-compression", 0i64);

        Self::finish_new(mpv, gl_area, auto_off)
    }

    #[cfg(not(target_os = "macos"))]
    fn finish_new(mut mpv: Mpv, gl_area: &gtk::GLArea, auto_off: bool) -> Result<(Self, bool), String> {
        let gl_libs = GlDynLib::load()?;
        let egl_state = EglState { get: gl_libs.get_proc };

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
                mpv,
                me_budget_shell_path: std::cell::RefCell::new(None),
                _gl: gl_libs,
                render,
                gl_ptr,
                pending_resume: std::cell::Cell::new(None),
                skip_media_persist: std::cell::Cell::new(false),
                warm_file_gen: std::cell::Cell::new(0),
                dvd_hold_global: std::cell::Cell::new(None),
                chapter_eof_load: std::cell::Cell::new(false),
                chapter_scrub_resume: std::cell::Cell::new(false),
                chapter_scrub_hold_pause: std::cell::Cell::new(false),
                chapter_scrub_unpause_after: std::cell::Cell::new(false),
            },
            auto_off,
        ))
    }

    #[cfg(target_os = "macos")]
    fn finish_new(mut mpv: Mpv, gl_area: &gtk::GLArea, auto_off: bool) -> Result<(Self, bool), String> {
        let macos = crate::mpv_embed::macos_video_bundle::MacosRender::install(&mut mpv, gl_area)?;
        Ok((
            Self {
                mpv,
                me_budget_shell_path: std::cell::RefCell::new(None),
                pending_resume: std::cell::Cell::new(None),
                skip_media_persist: std::cell::Cell::new(false),
                warm_file_gen: std::cell::Cell::new(0),
                dvd_hold_global: std::cell::Cell::new(None),
                chapter_eof_load: std::cell::Cell::new(false),
                chapter_scrub_resume: std::cell::Cell::new(false),
                chapter_scrub_hold_pause: std::cell::Cell::new(false),
                chapter_scrub_unpause_after: std::cell::Cell::new(false),
                macos: Some(macos),
            },
            auto_off,
        ))
    }

    mpv_bundle_macos_vf_methods!();

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

    /// macOS only: register a GTK widget whose visibility hides the native video layer.
    /// Call once after [`MpvBundle::new`] with the recent grid (or any overlay that GTK
    /// stacks on top of the GLArea) so closing the video reveals it.
    #[cfg(target_os = "macos")]
    pub fn watch_overlay<W: glib::object::IsA<gtk::Widget>>(&self, widget: &W) {
        if let Some(m) = self.macos.as_ref() {
            m.watch_overlay(widget);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn watch_overlay<W: glib::object::IsA<gtk::Widget>>(&self, _widget: &W) {}

    /// Continue-grid warm preload / post-resize: resync native layer frame; optional repin after shell resize.
    pub(crate) fn nudge_browse_video_layout(&self, gl: &gtk::GLArea) {
        self.nudge_video_layout(gl, false);
    }

    pub(crate) fn nudge_shell_layout_after_resize(&self, gl: &gtk::GLArea) {
        self.nudge_video_layout(gl, true);
    }

    fn nudge_video_layout(&self, gl: &gtk::GLArea, repin_gtk_stack: bool) {
        #[cfg(target_os = "macos")]
        if let Some(m) = self.macos.as_ref() {
            m.resync_layer_frame();
            if repin_gtk_stack {
                m.repin_below_gtk_compositing();
            }
        }
        gl.queue_render();
    }

    /// macOS only: clear the GLArea framebuffer with alpha=0 so the native video layer
    /// below shows through. Call from inside `connect_render`. Reuses the bundle's
    /// existing `OpenGL.framework` handle — no second `dlopen`.
    #[cfg(target_os = "macos")]
    pub fn clear_glarea_transparent(&self) {
        if let Some(m) = self.macos.as_ref() {
            m.clear_glarea_transparent();
        }
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
    /// Linux: run [`teardown_gl_paint`] with `gl_area` current earlier in the teardown chain;
    /// `dispose_for_quit` calls [`gtk::prelude::GLAreaExt::make_current`] again before freeing
    /// the render context and calling `mpv_terminate_destroy`.
    ///
    /// macOS: GLArea is a sizing placeholder only; the native render surface is freed before
    /// terminating mpv.
    #[cfg(not(target_os = "macos"))]
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

    #[cfg(target_os = "macos")]
    pub fn dispose_for_quit(mut self, _gl_area: &gtk::GLArea) {
        // Drop the native render surface first so its dispatch queue stops touching the
        // mpv render context before we tear it down.
        self.macos.take();
        let Self { mut mpv, .. } = self;
        mpv.set_wakeup_callback(|| {});
        unsafe {
            libmpv2_sys::mpv_terminate_destroy(mpv.ctx.as_ptr());
        }
        std::mem::forget(mpv);
    }
}
