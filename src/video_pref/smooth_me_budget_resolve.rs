// Effective bundled ME px²: per-file SQLite row, then closest-dimension neighbor, else global pref.

use std::path::PathBuf;

use libmpv2::Mpv;

use crate::media_probe::local_file_from_mpv;
use crate::mpv_embed::MpvBundle;

#[must_use]
pub(crate) fn me_budget_local_path(mpv: &Mpv, bundle: Option<&MpvBundle>) -> Option<PathBuf> {
    if let Some(b) = bundle {
        if let Some(p) = b.me_budget_shell_path.borrow().clone() {
            return Some(p);
        }
    }
    local_file_from_mpv(mpv)
}

#[must_use]
pub(crate) fn decode_wh_from_mpv(mpv: &Mpv) -> Option<(i32, i32)> {
    fn pair(mpv: &Mpv, wk: &str, hk: &str) -> Option<(i32, i32)> {
        let w = mpv.get_property::<i64>(wk).ok()?;
        let h = mpv.get_property::<i64>(hk).ok()?;
        (w > 0 && h > 0).then_some((w as i32, h as i32))
    }
    pair(mpv, "video-params/w", "video-params/h")
        .or_else(|| pair(mpv, "dwidth", "dheight"))
        .or_else(|| pair(mpv, "width", "height"))
}

/// ME budget for the current mpv media: [crate::db::resolve_media_smooth_me_budget].
#[must_use]
pub(crate) fn effective_smooth_me_budget_px(
    mpv: &Mpv,
    v: &crate::db::VideoPrefs,
    bundle: Option<&MpvBundle>,
) -> u64 {
    let global = v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let path = me_budget_local_path(mpv, bundle);
    let eff = crate::db::resolve_media_smooth_me_budget(path.as_deref(), global);
    if video_log() {
        let wh = decode_wh_from_mpv(mpv);
        let key = path
            .as_deref()
            .and_then(crate::db::history_key)
            .map(|s| s.len())
            .unwrap_or(0);
        let me_var = std::env::var(crate::paths::RHINO_SMOOTH_MAX_AREA_VAR).ok();
        let me_matches = crate::paths::smooth_max_area_env_matches(eff);
        eprintln!(
            "[rhino] video: (verbose) ME resolve effective_px²={eff} prefs.smooth_max_area={} mpv_decode_wh={wh:?} history_key_len={key} {}={me_var:?} max_area_env_matches={me_matches}",
            v.smooth_max_area,
            crate::paths::RHINO_SMOOTH_MAX_AREA_VAR,
        );
    }
    eff
}
