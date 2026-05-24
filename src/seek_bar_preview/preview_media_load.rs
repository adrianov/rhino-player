use std::path::Path;

use libmpv2::Mpv;

/// Stay inside the file: EOF on the auxiliary player leaves a black frame until reload.
const END_MARGIN_MIN_SEC: f64 = 1.0;
const END_MARGIN_MAX_SEC: f64 = 4.0;
const END_MARGIN_FRAC: f64 = 0.02;

/// Load target for the auxiliary player: same stream as main when mpv exposes a local file,
/// else shell/disc path (never rewrite a chapter `.m2ts` back to the full disc tree).
pub(crate) fn preview_load_path(main: &Mpv, shell: Option<&Path>) -> Option<String> {
    if let Ok(s) = main.get_property::<String>("path") {
        let t = s.trim();
        if t.starts_with("bd://") || t.starts_with("bluray://") {
            return Some(t.to_string());
        }
        if let Some(p) = crate::media_probe::local_path_from_mpv_str(t) {
            if p.is_file() && crate::video_ext::is_openable_media_path(&p) {
                return p.to_str().map(str::to_string);
            }
        }
    }
    let shell_p = crate::media_probe::shell_media_path(main, shell)?;
    let resolved = crate::video_ext::resolve_open_media_path(&shell_p);
    resolved.to_str().map(str::to_string)
}

#[must_use]
pub(crate) fn preview_cache_path(load: &str) -> std::path::PathBuf {
    if load.starts_with("bd://") || load.starts_with("bluray://") {
        std::path::PathBuf::from(load)
    } else {
        let p = Path::new(load);
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    }
}

fn preview_media_is_optical(load: &str) -> bool {
    if load.starts_with("bd://") || load.starts_with("bluray://") {
        return true;
    }
    let p = Path::new(load);
    crate::video_ext::is_optical_disc_path(p)
        || crate::video_ext::bluray_disc_root(p).is_some()
        || crate::video_ext::dvd_disc_root(p).is_some()
}

/// Bar length capped by main (and preview when known) duration so the right edge is not past EOF.
pub(crate) fn preview_hover_duration(
    bar_upper: f64,
    main: &Mpv,
    preview: Option<&Mpv>,
) -> f64 {
    if let Some(path) = crate::media_probe::local_file_from_mpv(main) {
        let live = main
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
            .unwrap_or(0.0);
        if let Some(bar) = crate::dvd_vob_timeline::DvdBarState::build(&path, live) {
            return bar.total_sec().min(bar_upper).max(0.0);
        }
    }
    let mut dur = bar_upper;
    if let Ok(d) = main.get_property::<f64>("duration") {
        if d.is_finite() && d > 0.0 {
            dur = dur.min(d);
        }
    }
    if let Some(pr) = preview {
        if let Ok(d) = pr.get_property::<f64>("duration") {
            if d.is_finite() && d > 0.0 {
                dur = dur.min(d);
            }
        }
    }
    dur.max(0.0)
}

#[must_use]
pub(crate) fn cap_preview_seek_time(t: f64, dur: f64) -> f64 {
    if !dur.is_finite() || dur <= 0.0 {
        return 0.0;
    }
    let margin = (dur * END_MARGIN_FRAC).clamp(END_MARGIN_MIN_SEC, END_MARGIN_MAX_SEC);
    t.clamp(0.0, (dur - margin).max(0.0))
}

fn hover_cap_duration(bar_upper: f64, main: Option<&Mpv>, preview: Option<&Mpv>) -> Option<f64> {
    if !(bar_upper.is_finite() && bar_upper > 0.0) {
        return None;
    }
    let main_dur = main
        .map(|m| preview_hover_duration(bar_upper, m, preview))
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(bar_upper);
    (main_dur > 0.0).then_some(main_dur)
}

/// Bar pointer x → seconds shown on the seek preview time label (and main seek on release).
#[must_use]
pub(crate) fn seek_bar_label_time(
    bar_upper: f64,
    bar_width: i32,
    x: f64,
    main: Option<&Mpv>,
    preview: Option<&Mpv>,
) -> Option<f64> {
    let main_dur = hover_cap_duration(bar_upper, main, preview)?;
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
    preview: Option<&Mpv>,
) -> Option<f64> {
    let main_dur = hover_cap_duration(bar_upper, main, preview)?;
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
pub(crate) fn preview_run_seek(mpv: &Mpv, t: f64, optical: bool) -> bool {
    let t_s = format!("{t:.3}");
    mpv.command("seek", &[t_s.as_str(), preview_seek_mode(optical)])
        .is_ok()
}

/// Lightweight decode for thumbnails; main-player `vf` / Bob deinterlace must not apply here.
pub(crate) fn prepare_preview_player(mpv: &Mpv, load: &str) {
    if preview_media_is_optical(load) {
        let _ = mpv.set_property("hwdec", "auto");
        let _ = mpv.set_property("hr-seek", "yes");
    } else {
        let _ = mpv.set_property("hwdec", "no");
        let _ = mpv.set_property("hr-seek", false);
    }
    let _ = mpv.command("vf", &["clr", ""]);
    set_preview_tracks(mpv);
}
