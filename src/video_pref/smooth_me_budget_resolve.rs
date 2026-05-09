// Effective bundled ME px²: per-file SQLite row, then closest-dimension neighbor, else global pref.

use libmpv2::Mpv;

use crate::media_probe::local_file_from_mpv;

#[must_use]
pub(crate) fn decode_wh_from_mpv(mpv: &Mpv) -> Option<(i32, i32)> {
    fn pair(mpv: &Mpv, wk: &str, hk: &str) -> Option<(i32, i32)> {
        let w = mpv.get_property::<i64>(wk).ok()?;
        let h = mpv.get_property::<i64>(hk).ok()?;
        (w > 0 && h > 0).then_some((w as i32, h as i32))
    }
    pair(mpv, "video-params/w", "video-params/h").or_else(|| pair(mpv, "width", "height"))
}

/// ME budget for the current mpv media: [crate::db::resolve_media_smooth_me_budget].
#[must_use]
pub(crate) fn effective_smooth_me_budget_px(mpv: &Mpv, v: &crate::db::VideoPrefs) -> u64 {
    let global = v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let path = local_file_from_mpv(mpv);
    let wh = decode_wh_from_mpv(mpv);
    let eff = crate::db::resolve_media_smooth_me_budget(path.as_deref(), wh, global);
    if video_log() {
        let key = path
            .as_deref()
            .and_then(crate::db::history_key)
            .map(|s| s.len())
            .unwrap_or(0);
        let snap_path = std::env::var(crate::paths::RHINO_SMOOTH_CAP_FILE_VAR).ok();
        let snap_matches = crate::paths::smooth_me_cap_snap_content_equals(eff);
        eprintln!(
            "[rhino] video: (verbose) ME resolve effective_px²={eff} prefs.smooth_max_area={} decode_wh={wh:?} history_key_len={key} {}={snap_path:?} snap_matches={snap_matches}",
            v.smooth_max_area,
            crate::paths::RHINO_SMOOTH_CAP_FILE_VAR,
        );
    }
    eff
}
