use std::cell::Cell;
use std::path::Path;

use libmpv2::Mpv;

/// Skip `seek` when mpv is already at the saved resume (warm preload applied it on `FileLoaded`).
const RESUME_AT_EPS: f64 = 1.0;

/// If mpv is near the start and SQLite has a resume, stash it unless already there.
pub(crate) fn stash_near_start_resume(mpv: &Mpv, pending: &Cell<Option<f64>>, path: &Path) {
    if pending.get().is_some() {
        return;
    }
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    if !(pos.is_finite() && pos < crate::media_probe::NEAR_END_SEC) {
        return;
    }
    let Some(t) = crate::db::resume_pos(path) else {
        return;
    };
    if !resume_already_at(mpv, t) {
        pending.set(Some(t));
    }
}

pub(crate) fn resume_already_at(mpv: &Mpv, target: f64) -> bool {
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    pos.is_finite() && target.is_finite() && (pos - target).abs() < RESUME_AT_EPS
}

pub(crate) fn seek_to_resume_sec(mpv: &Mpv, t: f64) {
    let _ = crate::video_pref::unload_smooth_on_pause(mpv);
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+exact"]);
}
