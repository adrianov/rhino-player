use std::path::Path;
use std::sync::atomic::AtomicU64;

use crate::db;
use crate::db::VideoPrefs;
use crate::paths;
use crate::paths::{
    publish_smooth_me_budget_env, smooth_max_area_env_matches, RHINO_PLAYBACK_SPEED_VAR,
    RHINO_VPY_LOG_EPOCH_VAR,
};
use crate::playback_speed::MAX_FIXED_SPEED;

include!("mvtools_vf_substring_checks.rs");

/// [apply_mpv_video] result (replaces a bare `bool` for "smooth was auto-off" on older call sites).
#[derive(Debug, Default)]
pub struct MpvVideoApply {
    /// Prefs had **Smooth 60** turned off (missing script, `vf` rejected, etc.).
    pub smooth_auto_off: bool,
}

/// Monotonic epoch published per `vf add` so the bundled `.vpy` can tag its log lines.
static VPY_LOG_EPOCH: AtomicU64 = AtomicU64::new(0);
include!("mvtools_speed_env_sync.rs");

/// True when mpv's `vf` chain already matches what [add_smooth_60] would install for current prefs
/// (resolved script · **`buffered-frames`** · **`concurrent-frames=auto`** · bundled **`RHINO_SMOOTH_MAX_AREA`** env ·
/// **`smooth_vf_me_budget_applied`**). Used to skip redundant **`vf clr`**/**`vf add`** on duplicate idle
/// after **FileLoaded** / **`path`** / debounced post-**seek** resync (see **`schedule_smooth_60_resync_idle`**).
pub(crate) fn vf_smooth_matches_prefs(
    mpv: &Mpv,
    v: &VideoPrefs,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    if !v.smooth_60 {
        return false;
    }
    let Some(script) = resolve_vs_script_path(v) else {
        return false;
    };
    let Ok(vf) = mpv.get_property::<String>("vf") else {
        return false;
    };
    let vfl = vf.to_lowercase();
    vfl.contains("vapoursynth")
        && vf_smooth_script_matches(&vf, &script)
        && vf_smooth_opts_match(mpv, &vf, bundle)
        && vf_smooth_budget_env_matches(mpv, v, bundle)
}

/// Script check against the current `vf`: escaped absolute path, raw path, or bare file name.
fn vf_smooth_script_matches(vf: &str, script_path: &str) -> bool {
    let script = script_path.trim();
    let esc = mpv_escape_path(script);
    let base_matches = std::path::Path::new(script)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|base| vf.contains(base));
    vf.contains(&esc) || vf.contains(script) || base_matches
}

/// Fixed queue depth + `concurrent-frames=auto`, plus Blu-ray bob-deinterlace when wanted.
fn vf_smooth_opts_match(mpv: &Mpv, vf: &str, bundle: Option<&crate::mpv_embed::MpvBundle>) -> bool {
    if wants_bluray_bob_deinterlace(mpv, bundle) && !bluray_deinterlace_in_vf(vf) {
        return false;
    }
    vf_smooth_queue_chain_ok(vf) && vf_concurrent_frames_matches(vf, "auto")
}

/// Bundled ME px² env / applied-marker agree with current prefs for the open media.
fn vf_smooth_budget_env_matches(
    mpv: &Mpv,
    v: &VideoPrefs,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    let me_cap = effective_smooth_me_budget_px(mpv, v, bundle);
    (!v.vs_path.trim().is_empty() || smooth_max_area_env_matches(me_cap))
        && bundled_me_budget_vf_matches_prefs(mpv, v, bundle)
}

/// True when **`vf`** carries the fixed **`buffered-frames`** depth (**[SMOOTH_VF_BUFFERED_FRAMES]**).
pub(crate) fn vf_smooth_queue_chain_ok(vf: &str) -> bool {
    vf.contains(&format!("buffered-frames={}", SMOOTH_VF_BUFFERED_FRAMES))
}

fn resolve_vs_script_path(v: &VideoPrefs) -> Option<String> {
    let t = v.vs_path.trim();
    if !t.is_empty() {
        return if Path::new(t).is_file() {
            Some(t.to_string())
        } else {
            eprintln!("[rhino] video: VapourSynth path is not a file: {t}");
            None
        };
    }
    paths::bundled_mvtools_60().and_then(|b| b.to_str().map(|s| s.to_string()))
}

fn turn_off_smooth_60_in_prefs(v: &mut VideoPrefs) {
    v.smooth_60 = false;
    db::save_video(v);
}

/// After `vf` is cleared, add ~60 fps filter when [VideoPrefs::smooth_60]. Returns `true` if we
/// **disabled** the option in prefs (VapourSynth path missing and no bundle, or `vf` add failed).
/// True when a media file is open (filters must attach after [loadfile] so `video_in` exists).
pub(crate) fn mpv_has_open_media(mpv: &Mpv) -> bool {
    // `path` is the main/selected file; empty before the first `loadfile` or while idle.
    matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty())
}

// `prep_smooth_60_for_vf` + `add_smooth_60` live in `smooth_vf_add.rs`.
