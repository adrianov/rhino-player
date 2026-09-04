use std::cell::Cell;
use std::path::Path;

use libmpv2::Mpv;

/// Skip `seek` when mpv is already at the saved resume (warm preload applied it on `FileLoaded`).
pub(crate) const RESUME_AT_EPS: f64 = 1.0;

/// If mpv is near the start and SQLite has a resume, stash it unless already there.
pub(crate) fn stash_near_start_resume(mpv: &Mpv, pending: &Cell<Option<f64>>, path: &Path) {
    if pending.get().is_some() {
        return;
    }
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    if !(pos.is_finite() && pos < crate::media_probe::NEAR_END_SEC) {
        return;
    }
    let Some((target, local)) = stored_resume_target(path) else {
        return;
    };
    if !crate::video_ext::paths_same_file(&target, path) {
        return;
    }
    if !resume_already_at(mpv, local) {
        pending.set(Some(local));
    }
}

/// SQLite resume for `path`'s entity remapped through the loader: `(load target, local secs)`.
pub(crate) fn stored_resume_target(path: &Path) -> Option<(std::path::PathBuf, f64)> {
    let entity = crate::playback_entity::PlaybackEntity::resolve(path);
    let t = crate::db::resume_pos(&entity.db_path())?;
    entity.resume_load_target(path, t, &crate::db::load_duration_map())
}

pub(crate) fn resume_already_at(mpv: &Mpv, target: f64) -> bool {
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    pos.is_finite() && target.is_finite() && (pos - target).abs() < RESUME_AT_EPS
}

pub(crate) fn seek_to_resume_sec(mpv: &Mpv, t: f64) {
    let _ = crate::video_pref::unload_smooth_for_seek(mpv, None);
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+exact"]);
}

pub(crate) fn seek_chain_ifo_local(mpv: &Mpv, chapter: &Path, ifo_local: f64) {
    let mpv_t = crate::dvd_vob_timeline::chain_head_ifo_seg(chapter)
        .map(|seg| crate::dvd_vob_timeline::chain_head_mpv_seek_sec(mpv, ifo_local, seg))
        .unwrap_or(ifo_local);
    crate::dvd_vob_log::dvd_seek_log(format!(
        "chain seek ifo={ifo_local:.2} -> mpv={mpv_t:.2} ({})",
        chapter.file_name().and_then(|s| s.to_str()).unwrap_or("?")
    ));
    seek_to_resume_sec(mpv, mpv_t);
}

pub(crate) fn resume_already_at_ifo(mpv: &Mpv, chapter: &Path, ifo_local: f64) -> bool {
    match crate::dvd_vob_timeline::chain_head_ifo_seg(chapter) {
        None => resume_already_at(mpv, ifo_local),
        Some(seg) => stretched_tail_at(mpv, seg, ifo_local),
    }
}

/// Stretched chain-head comparison: map the live tail position back to IFO-local seconds;
/// unstretched heads fall back to the plain seconds comparison.
fn stretched_tail_at(mpv: &Mpv, seg: f64, ifo_local: f64) -> bool {
    let Some(dur) = stretched_head_duration(mpv, seg) else {
        return resume_already_at(mpv, ifo_local);
    };
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    if pos < crate::dvd_vob_timeline::chain_head_tail(dur, seg) - 0.5 {
        return false;
    }
    let ifo = crate::dvd_vob_timeline::chain_head_ifo_local_from_mpv(pos, dur, seg);
    ifo.is_finite() && (ifo - ifo_local).abs() < RESUME_AT_EPS
}

/// Positive finite head **`duration`** when the chain head is stretched; None otherwise.
fn stretched_head_duration(mpv: &Mpv, seg: f64) -> Option<f64> {
    let dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    crate::dvd_vob_timeline::chain_head_stretched(dur, seg).then_some(dur)
}
