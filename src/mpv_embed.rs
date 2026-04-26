//! libmpv OpenGL output in a [`gtk::GLArea`]. See `docs/features/03-mpv-embedding.md` and `docs/product-and-use-cases.md`.

use glib::prelude::Cast;
use glib::translate::from_glib_borrow;
use gtk::prelude::*;
use libloading::{Library, Symbol};
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::path::Path;
use std::ptr;

use crate::db::VideoPrefs;
use crate::media_probe;
use crate::paths;
use crate::video_pref::apply_mpv_video;

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
    /// [VideoPrefs] (optional VapourSynth 60 fps `vf`) from SQLite; see [apply_mpv_video].
    /// The `bool` is `true` when **Smooth video (~60 FPS at 1.0×)** was auto-disabled (VapourSynth `vf` rejected); sync UI.
    pub fn new(gl_area: &gtk::GLArea, video: &mut VideoPrefs) -> Result<(Self, bool), String> {
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
            init.set_option("osc", "no")?;
            // 0 = auto: libavcodec can use multiple CPU threads for software decode
            // (independent of heavy single-threaded sections in some filters / MVTools).
            let _ = init.set_option("vd-lavc-threads", "0");
            let _ = init.set_option("ao", "pulse");
            let _ = init.set_option("keep-open", "yes");
            if let Some(ref dir) = paths::watch_later() {
                if let Some(s) = dir.to_str() {
                    let _ = init.set_option("save-position-on-quit", "yes");
                    let _ = init.set_option("watch-later-dir", s);
                    // Store path text in the watch_later file so keys match the same file across opens.
                    let _ = init.set_option("write-filename-in-watch-later-config", "yes");
                }
            }
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;

        // Re-assert: some init options apply more reliably as properties on the open handle.
        let _ = mpv.set_property("save-position-on-quit", true);
        let auto_off = apply_mpv_video(&mpv, video, None).smooth_auto_off;
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
                _egl,
                _gl,
                gl_get,
                mpv,
                render,
                gl_ptr,
            },
            auto_off,
        ))
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

    /// End playback; call after watch-later / DB snapshot. Safe to skip before process exit.
    pub fn stop_playback(&self) {
        let _ = self.mpv.command("stop", &[]);
    }

    /// Write the current file’s state into `watch_later` (separate from shutdown-time save).
    pub fn write_resume_snapshot(&self) {
        let _ = self.mpv.command("write-watch-later-config", &[]);
    }

    /// Duration + position for the recent grid (local files only); thumbnails refresh when the grid is shown.
    pub fn save_playback_state(&self) {
        media_probe::record_playback_for_current(&self.mpv);
    }

    /// Mute+pause to silence immediately, then write `watch_later` with the **prior** mute/pause so
    /// the next run does not resume muted or paused. Re-hush for thumbnail I/O, then [stop].
    /// If playback was at the **natural end**, resume data is [cleared](media_probe::clear_resume_for_path) instead of saved.
    pub fn commit_quit(&self) {
        let at_end = media_probe::is_natural_end(&self.mpv);
        if at_end {
            if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
                media_probe::clear_resume_for_path(&p);
            }
        }
        let mute0 = self.mpv.get_property::<bool>("mute").unwrap_or(false);
        let pause0 = self.mpv.get_property::<bool>("pause").unwrap_or(false);
        let _ = self.mpv.set_property("mute", true);
        let _ = self.mpv.set_property("pause", true);
        let _ = self.mpv.set_property("mute", mute0);
        let _ = self.mpv.set_property("pause", pause0);
        if !at_end {
            let _ = self.mpv.command("write-watch-later-config", &[]);
        }
        let _ = self.mpv.set_property("mute", true);
        let _ = self.mpv.set_property("pause", true);
        self.save_playback_state();
        self.stop_playback();
    }

    /// Write resume for the current file, or clear it if playback finished (EOF / within ~3s of end).
    pub fn snapshot_outgoing_before_leave(&self) {
        if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
            if media_probe::is_natural_end(&self.mpv) {
                media_probe::clear_resume_for_path(&p);
            } else {
                let _ = self.mpv.command("write-watch-later-config", &[]);
                media_probe::record_playback_for_current(&self.mpv);
            }
        } else {
            let _ = self.mpv.command("write-watch-later-config", &[]);
            media_probe::record_playback_for_current(&self.mpv);
        }
    }

    /// Save the current file’s position, then `loadfile` the new path. Uses a **canonical** path
    /// string so the watch_later index matches the next open of the same file. Resume from the
    /// previous session is **only** from libmpv’s `watch_later` + `resume-playback` (we do not pass
    /// `start=` in `loadfile`—combining that with the same sidecar can double-apply the offset and
    /// cause **slight A/V desync**).
    /// When [clear_outgoing_resume] is true, the outgoing file reached the end: drop watch_later + DB
    /// position (next open from 0) and do not write a final end snapshot.
    pub fn load_file_path(&self, path: &Path, clear_outgoing_resume: bool) -> Result<(), String> {
        if clear_outgoing_resume {
            if let Some(p) = media_probe::local_file_from_mpv(&self.mpv) {
                media_probe::clear_resume_for_path(&p);
            }
        } else {
            self.write_resume_snapshot();
            media_probe::record_playback_for_current(&self.mpv);
        }
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
        self.mpv
            .command("loadfile", &[s, "replace"])
            .map_err(|e| format!("{e:?}"))
    }
}

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
        let _ = self.render.render::<EglState>(fbo, w, h, true);
        self.render.report_swap();
    }
}

pub fn set_preview_tracks(mpv: &Mpv) {
    let _ = mpv.set_property("aid", "no");
    let _ = mpv.set_property("sid", "no");
    let _ = mpv.set_property("secondary-sid", "no");
    let _ = mpv.set_property("pause", true);
}
