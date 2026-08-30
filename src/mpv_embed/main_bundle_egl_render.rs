use glib::prelude::Cast;
use glib::translate::from_glib_borrow;
use gtk::prelude::*;
pub use libmpv2::events::{Event, PropertyData};
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
pub use libmpv2::Format;
use libmpv2::Mpv;
use std::path::Path;

use crate::db;
use crate::db::VideoPrefs;
use crate::media_probe;
use crate::video_pref::apply_mpv_video_init;
use gl_platform::GlDynLib;

// EGL helper types (`EglState`, `egl_proc`, `GL_FRAMEBUFFER_BINDING`) live in
// `mpv_embed/linux_egl_helpers.rs` and are included into the same module.
// The Linux EGL render pipeline lives in `mpv_bundle_egl_linux.rs`.

fn wait_event_err_is_load_fail(err: &libmpv2::Error) -> bool {
    match err {
        libmpv2::Error::Raw(code) => matches!(
            *code,
            libmpv2::mpv_error::LoadingFailed
                | libmpv2::mpv_error::NothingToPlay
                | libmpv2::mpv_error::UnknownFormat
        ),
        _ => false,
    }
}
/// `MpvBundle` cell initializers shared verbatim by both platform constructors. Expands to a
/// whole `Self { … }`; `$($platform_fields:tt)*` appends the platform-specific fields.
macro_rules! mpv_bundle_self {
    ($mpv:ident, $($platform_fields:tt)*) => {
        Self {
            mpv: $mpv,
            me_budget_shell_path: std::cell::RefCell::new(None),
            pending_resume: std::cell::Cell::new(None),
            skip_media_persist: std::cell::Cell::new(false),
            warm_file_gen: std::cell::Cell::new(0),
            dvd_hold_global: std::cell::Cell::new(None),
            dvd_chain_bar_sync: std::cell::Cell::new(None),
            transport_bar_total: std::cell::Cell::new(None),
            transport_bar_global: std::cell::Cell::new(None),
            chapter_eof_load: std::cell::Cell::new(false),
            chapter_scrub_resume: std::cell::Cell::new(false),
            chapter_scrub_hold_pause: std::cell::Cell::new(false),
            chapter_scrub_unpause_after: std::cell::Cell::new(false),
            smooth_vf_attach_pending: std::cell::Cell::new(false),
            smooth_vf_stripped_this_open: std::cell::Cell::new(false),
            smooth_vf_reload_attempted: std::cell::Cell::new(false),
            $($platform_fields)*
        }
    };
}

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
    /// Chain-head `.vob`: mpv `time-pos` offset vs title-wide bar (see [crate::dvd_vob_timeline::DvdChainBarSync]).
    pub(crate) dvd_chain_bar_sync:
        std::cell::Cell<Option<crate::dvd_vob_timeline::DvdChainBarSync>>,
    /// Last title-wide bar `(total, global)` from [crate::app::transport_events] — used when persisting DVD entity rows.
    transport_bar_total: std::cell::Cell<Option<f64>>,
    transport_bar_global: std::cell::Cell<Option<f64>>,
    /// Title-internal chapter `loadfile` from DVD EOF advance (keep vf, unpause after load).
    pub(crate) chapter_eof_load: std::cell::Cell<bool>,
    /// Cross-chapter unified-bar scrub: chapter-local [pending_resume]; ignore SQLite near-start.
    chapter_scrub_resume: std::cell::Cell<bool>,
    /// Hold `pause=yes` until cross-chapter resume seek lands (avoids playing from file start).
    chapter_scrub_hold_pause: std::cell::Cell<bool>,
    /// Unpause when hold ends when [load_chapter_seek] was called with `resume_playing=true`.
    chapter_scrub_unpause_after: std::cell::Cell<bool>,
    /// True between **`vf add vapoursynth`** and mpv reporting the filter in **`vf`** (coalesces duplicate applies).
    smooth_vf_attach_pending: std::cell::Cell<bool>,
    /// Set when **`vf remove vapoursynth`** ran on the current open media (mpv often rejects immediate re-add).
    smooth_vf_stripped_this_open: std::cell::Cell<bool>,
    /// One **`loadfile replace`** per strip cycle so a failed **`vf add`** does not loop reloads.
    smooth_vf_reload_attempted: std::cell::Cell<bool>,

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
        let mpv = Mpv::with_initializer(Self::mpv_init_options).map_err(|e| format!("{e:?}"))?;

        let auto_off = apply_mpv_video_init(&mpv, video).smooth_auto_off;
        // Thumbnails: prefer JPEG (fast); PNG path uses minimum compression.
        let _ = mpv.set_property("screenshot-format", "jpeg");
        let _ = mpv.set_property("screenshot-jpeg-quality", 90i64);
        let _ = mpv.set_property("screenshot-png-compression", 0i64);

        Self::finish_new(mpv, gl_area, auto_off)
    }

    /// Player-wide options applied during mpv core initialization (shared by both platforms).
    fn mpv_init_options(init: libmpv2::MpvInitializer) -> Result<(), libmpv2::Error> {
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
    }

    #[cfg(target_os = "macos")]
    fn finish_new(
        mut mpv: Mpv,
        gl_area: &gtk::GLArea,
        auto_off: bool,
    ) -> Result<(Self, bool), String> {
        let macos = crate::mpv_embed::macos_video_bundle::MacosRender::install(&mut mpv, gl_area)?;
        Ok((mpv_bundle_self!(mpv, macos: Some(macos)), auto_off))
    }

    mpv_bundle_macos_vf_methods!();
}

include!("mpv_bundle_egl_linux.rs");
include!("mpv_bundle_drain_layout.rs");
