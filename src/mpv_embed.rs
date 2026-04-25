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

use crate::media_probe;
use crate::paths;

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
            init.set_option("osc", "no")?;
            let _ = init.set_option("ao", "pulse");
            let _ = init.set_option("keep-open", "yes");
            // Smooth presentation on 60+ Hz fixed displays: re-time frames to display refresh (not SOFI).
            let _ = init.set_option("video-sync", "display-resample");
            let _ = init.set_option("interpolation", "yes");
            let _ = init.set_option("tscale", "oversample");
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
        let _ = mpv.set_property("video-sync", "display-resample");
        let _ = mpv.set_property("interpolation", true);
        let _ = mpv.set_property("tscale", "oversample");
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

    /// End playback; call after watch-later / DB snapshot. Safe to skip before process exit.
    pub fn stop_playback(&self) {
        let _ = self.mpv.command("stop", &[]);
    }

    /// Write the current file’s state into `watch_later` (separate from shutdown-time save).
    pub fn write_resume_snapshot(&self) {
        let _ = self.mpv.command("write-watch-later-config", &[]);
    }

    /// Last frame + duration for the recent grid (local files only); see [media_probe::persist_on_quit].
    pub fn persist_on_quit(&self) {
        media_probe::persist_on_quit(&self.mpv);
    }

    /// Mute+pause to silence immediately, then write `watch_later` with the **prior** mute/pause so
    /// the next run does not resume muted or paused. Re-hush for thumbnail I/O, then [stop].
    pub fn commit_quit(&self) {
        let mute0 = self.mpv.get_property::<bool>("mute").unwrap_or(false);
        let pause0 = self.mpv.get_property::<bool>("pause").unwrap_or(false);
        let _ = self.mpv.set_property("mute", true);
        let _ = self.mpv.set_property("pause", true);
        let _ = self.mpv.set_property("mute", mute0);
        let _ = self.mpv.set_property("pause", pause0);
        let _ = self.mpv.command("write-watch-later-config", &[]);
        let _ = self.mpv.set_property("mute", true);
        let _ = self.mpv.set_property("pause", true);
        self.persist_on_quit();
        self.stop_playback();
    }

    /// Save the current file’s position, then `loadfile` the new path. Uses a **canonical** path
    /// string so the watch_later index matches the next open of the same file.
    pub fn load_file_path(&self, path: &Path) -> Result<(), String> {
        self.write_resume_snapshot();
        media_probe::record_playback_for_current(&self.mpv);
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let s = canonical.to_str().ok_or("media path is not valid UTF-8")?;
        self.mpv
            .command("loadfile", &[s, "replace"])
            .map_err(|e| format!("{e:?}"))
    }
}

