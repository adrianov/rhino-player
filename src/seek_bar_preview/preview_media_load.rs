use std::path::Path;

use libmpv2::Mpv;

/// Stay inside the file: EOF on the auxiliary player leaves a black frame until reload.
const END_MARGIN_MIN_SEC: f64 = 1.0;
const END_MARGIN_MAX_SEC: f64 = 4.0;
const END_MARGIN_FRAC: f64 = 0.02;

#[must_use]
pub(crate) fn preview_cache_path(load: &str) -> std::path::PathBuf {
    if load.starts_with("bd://") || load.starts_with("bluray://") {
        std::path::PathBuf::from(load)
    } else {
        let p = Path::new(load);
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    }
}

pub(crate) fn preview_media_is_optical(load: &str) -> bool {
    if load.starts_with("bd://") || load.starts_with("bluray://") {
        return true;
    }
    let p = Path::new(load);
    crate::video_ext::is_optical_disc_path(p)
        || crate::video_ext::bluray_disc_root(p).is_some()
        || crate::video_ext::dvd_disc_root(p).is_some()
}

#[must_use]
pub(crate) fn cap_preview_seek_time(t: f64, dur: f64) -> f64 {
    if !dur.is_finite() || dur <= 0.0 {
        return 0.0;
    }
    let margin = (dur * END_MARGIN_FRAC).clamp(END_MARGIN_MIN_SEC, END_MARGIN_MAX_SEC);
    t.clamp(0.0, (dur - margin).max(0.0))
}

fn hover_cap_duration(
    bar_upper: f64,
    main: Option<&Mpv>,
    shell: Option<&Path>,
    preview: Option<&Mpv>,
    dvd_bar: Option<&std::cell::RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
) -> Option<f64> {
    if !(bar_upper.is_finite() && bar_upper > 0.0) {
        return None;
    }
    let main = main?;
    crate::playback_entity::preview_hover_duration_for_open(
        main,
        shell,
        bar_upper,
        preview,
        dvd_bar,
    )
    .filter(|d| d.is_finite() && *d > 0.0)
}

/// Bar pointer x → seconds shown on the seek preview time label (and main seek on release).
#[must_use]
pub(crate) fn seek_bar_label_time(
    bar_upper: f64,
    bar_width: i32,
    x: f64,
    main: Option<&Mpv>,
    shell: Option<&Path>,
    preview: Option<&Mpv>,
    dvd_bar: Option<&std::cell::RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
) -> Option<f64> {
    let main_dur = hover_cap_duration(bar_upper, main, shell, preview, dvd_bar)?;
    let w = f64::from(bar_width.max(1));
    let raw = (x / w).clamp(0.0, 1.0) * bar_upper;
    Some(cap_preview_seek_time(raw, main_dur))
}

/// Scale thumb value → same capped seconds as [seek_bar_label_time].
#[must_use]
pub(crate) fn seek_bar_label_time_from_value(
    bar_upper: f64,
    value: f64,
    main: Option<&Mpv>,
    shell: Option<&Path>,
    preview: Option<&Mpv>,
    dvd_bar: Option<&std::cell::RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
) -> Option<f64> {
    let main_dur = hover_cap_duration(bar_upper, main, shell, preview, dvd_bar)?;
    Some(cap_preview_seek_time(value.clamp(0.0, bar_upper), main_dur))
}

fn preview_seek_mode(optical: bool) -> &'static str {
    if optical {
        "absolute+exact"
    } else {
        "absolute+keyframes"
    }
}

/// Run seek on the auxiliary player; returns false on mpv error.
pub(crate) fn preview_run_seek(mpv: &Mpv, load: &str, ifo_t: f64, optical: bool) -> bool {
    let chapter = Path::new(load);
    let t = if optical && chapter.is_file() {
        crate::dvd_vob_timeline::preview_mpv_seek_sec(chapter, ifo_t, mpv)
    } else {
        ifo_t
    };
    let t_s = format!("{t:.3}");
    mpv.command("seek", &[t_s.as_str(), preview_seek_mode(optical)])
        .is_ok()
}

/// Downscale decode so hover scrub does not contend with main Smooth / VapourSynth decode.
const PREVIEW_DECODE_MAX_W: i32 = 480;

/// Lightweight decode for thumbnails; main-player `vf` / Bob deinterlace must not apply here.
pub(crate) fn prepare_preview_player(mpv: &Mpv, load: &str) {
    // Software decode only — `hwdec=auto` on DVD `.vob` has broken `vo=libmpv` on macOS.
    let _ = mpv.set_property("hwdec", "no");
    if preview_media_is_optical(load) {
        let _ = mpv.set_property("hr-seek", "yes");
        let _ = mpv.command("vf", &["clr", ""]);
    } else {
        let _ = mpv.set_property("hr-seek", false);
        let w = PREVIEW_DECODE_MAX_W.to_string();
        let scale = format!("scale={w}:-2");
        let _ = mpv.command("vf", &["clr", ""]);
        let _ = mpv.command("vf", &["append", scale.as_str()]);
    }
    crate::mpv_embed::set_preview_tracks(mpv);
}

/// Revert preview mpv decode prefs without GL calls (safe during main `loadfile`).
pub(crate) fn reset_preview_player_decode(mpv: &Mpv) {
    let _ = mpv.set_property("hwdec", "no");
    let _ = mpv.set_property("hr-seek", false);
    let _ = mpv.command("vf", &["clr", ""]);
    crate::mpv_embed::set_preview_tracks(mpv);
}
