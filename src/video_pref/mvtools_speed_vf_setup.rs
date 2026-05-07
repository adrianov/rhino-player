use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use libmpv2::Mpv;

use crate::db;
use crate::db::VideoPrefs;
use crate::db::MIN_SMOOTH_MAX_AREA;
use crate::paths;
use crate::paths::{
    RHINO_PLAYBACK_SPEED_VAR, RHINO_SMOOTH_MAX_AREA_VAR, RHINO_VPY_LOG_EPOCH_VAR,
};
use crate::playback_speed::MAX_FIXED_SPEED;

/// [apply_mpv_video] result (replaces a bare `bool` for "smooth was auto-off" on older call sites).
#[derive(Debug, Default)]
pub struct MpvVideoApply {
    /// Prefs had **Smooth 60** turned off (missing script, `vf` rejected, etc.).
    pub smooth_auto_off: bool,
}

/// “≈1.0×” band: bundled mvtools [vf] eligibility and env comparison use this tolerance.
const PLAYBACK_1X_EPS: f64 = 0.001;

static VPY_LOG_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Same string [mpv] and the VapourSynth script use for `RHINO_PLAYBACK_SPEED`.
fn normalized_env_speed(s: f64) -> f64 {
    if !s.is_finite() {
        return 1.0;
    }
    // Cap at the fastest fixed UI step so env matches mpv (see playback_speed::MAX_FIXED_SPEED).
    let s = if s > 0.01 && s <= MAX_FIXED_SPEED { s } else { 1.0 };
    (s * 10.0).round() / 10.0
}

/// Set [paths::RHINO_PLAYBACK_SPEED_VAR] to `speed` (e.g. value just sent with [Mpv] `set_property`,
/// before [get_property] reflects it — avoids a stale env when rebuilding the [vf]).
pub fn set_playback_speed_env(speed: f64) {
    let t = normalized_env_speed(speed);
    std::env::set_var(RHINO_PLAYBACK_SPEED_VAR, format!("{t}"));
}

/// Set [paths::RHINO_PLAYBACK_SPEED_VAR] from [libmpv2::Mpv] `speed` (defaults to `1.0`). Used before
/// loading the VapourSynth filter so the bundled script matches interpolation to (source fps × speed).
pub fn set_playback_speed_env_from_mpv(mpv: &Mpv) {
    let s = match mpv.get_property::<f64>("speed") {
        Ok(v) if v.is_finite() => v,
        _ => 1.0,
    };
    set_playback_speed_env(s);
}

/// Bundled mvtools / FlowFPS is only used at **1.0×** (no speed-up). If [mpv] `speed` is not ~1, the
/// [vf] is omitted; **Smooth 60** pref may stay on for when the user returns to 1.0×.
/// [speed_hint] is used when [Some] (e.g. header row) so we do not read [get_property] before it matches
/// the value just sent with [set_property] — that race skipped re-adding the [vf] when going 1.5/2.0 → 1.0.
pub(crate) fn mvtools_vf_eligible(mpv: &Mpv, speed_hint: Option<f64>) -> bool {
    let s = match speed_hint {
        Some(x) if x.is_finite() => normalized_env_speed(x),
        _ => match mpv.get_property::<f64>("speed") {
            Ok(v) if v.is_finite() => normalized_env_speed(v),
            _ => 1.0,
        },
    };
    (s - 1.0).abs() <= PLAYBACK_1X_EPS
}

/// `true` when the process env disagrees with current [mpv] `speed` (e.g. [vf] added before resume
/// applied playback speed, or UI set `speed` before the resync read ran).
pub fn needs_playback_speed_env_resync(mpv: &Mpv) -> bool {
    let want = {
        let s = match mpv.get_property::<f64>("speed") {
            Ok(v) if v.is_finite() => v,
            _ => 1.0,
        };
        normalized_env_speed(s)
    };
    let have = std::env::var(RHINO_PLAYBACK_SPEED_VAR)
        .ok()
        .and_then(|t| t.parse::<f64>().ok())
        .map(normalized_env_speed)
        .unwrap_or(1.0);
    (have - want).abs() > PLAYBACK_1X_EPS
}

/// If **Smooth 60** is on and media is open, runs [apply_mpv_video] when the decode/`vf` state should
/// change: env/`speed` mismatch, or the graph does not match [mvtools_vf_eligible] (want **vapoursynth**
/// only at ~1.0×; strip when sped up), or the loaded `vf` does not match prefs/script/buffer options.
/// Returns the same shape as [apply_mpv_video].
pub fn resync_smooth_if_speed_mismatch(
    b: &crate::mpv_embed::MpvBundle,
    v: &mut VideoPrefs,
) -> MpvVideoApply {
    let mpv = &b.mpv;
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return MpvVideoApply::default();
    }
    let want_mvtools = mvtools_vf_eligible(mpv, None);
    let has = vf_chain_has_vapoursynth(mpv);
    let graph_ok = !has || vf_smooth_matches_prefs(mpv, v);
    if !needs_playback_speed_env_resync(mpv) && want_mvtools == has && graph_ok {
        return MpvVideoApply::default();
    }
    apply_mpv_video(b, v, None)
}

/// After [libmpv2::Mpv] `speed` changes: re-run [apply_mpv_video] so `vf` / decode match
/// (mvtools only at ~1.0×; see [mvtools_vf_eligible]).
/// Pass [speed_hint] with the `speed` you just set in mpv to avoid a **get_property** race; use `None` to
/// read the current [mpv] value.
pub fn refresh_smooth_for_playback_speed(
    b: &crate::mpv_embed::MpvBundle,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    let mpv = &b.mpv;
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: video pipeline resync for playback speed");
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    apply_mpv_video(b, v, speed_hint)
}

/// True when mpv's `vf` chain already matches what [add_smooth_60] would install for current prefs
/// (same script path / basename and fixed queue settings). Used to skip redundant **`vf clr`**/**`vf add`**
/// when transport fires duplicate idle callbacks after **FileLoaded** / **`path`** — **seek** never reaches
/// [apply_mpv_video_impl].
pub(crate) fn vf_smooth_matches_prefs(mpv: &Mpv, v: &VideoPrefs) -> bool {
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
    if !vfl.contains("vapoursynth") {
        return false;
    }
    let bf = format!("buffered-frames={}", SMOOTH_VF_BUFFERED_FRAMES);
    if !vf.contains(&bf) || !vf.contains("concurrent-frames=auto") {
        return false;
    }
    let script = script.trim();
    let esc = mpv_escape_path(script);
    let path_matches = vf.contains(&esc) || vf.contains(script);
    let base_matches = std::path::Path::new(script)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|base| vf.contains(base));
    if !(path_matches || base_matches) {
        return false;
    }
    smooth_max_area_env_matches(v)
}

fn smooth_max_area_env_matches(v: &VideoPrefs) -> bool {
    let want = v.smooth_max_area.max(MIN_SMOOTH_MAX_AREA);
    std::env::var(RHINO_SMOOTH_MAX_AREA_VAR)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        == Some(want)
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

fn add_smooth_60(mpv: &Mpv, v: &mut VideoPrefs, speed_hint: Option<f64>) -> bool {
    if !v.smooth_60 {
        return false;
    }
    if !mpv_has_open_media(mpv) {
        // Init-time [apply_mpv_video] and pre-load calls must *not* run `vf add` (no `video_in` / no
        // `path` yet) — a failed add used to look like a broken install and **disabled 60p in the DB**.
        eprintln!(
            "[rhino] video: VapourSynth deferred (no `path` yet — will apply after loadfile)"
        );
        return false;
    }
    if !mvtools_vf_eligible(mpv, speed_hint) {
        return false;
    }
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    std::env::set_var(
        RHINO_SMOOTH_MAX_AREA_VAR,
        format!("{}", v.smooth_max_area.max(MIN_SMOOTH_MAX_AREA)),
    );
    set_source_fps_env_from_mpv(mpv);
    if video_log() {
        eprintln!(
            "[rhino] video: (verbose) buffered-frames={}",
            SMOOTH_VF_BUFFERED_FRAMES
        );
    }
    let epoch = VPY_LOG_EPOCH.fetch_add(1, Ordering::Relaxed);
    std::env::set_var(RHINO_VPY_LOG_EPOCH_VAR, format!("{epoch}"));
    if !apply_mvtools_env(v) {
        turn_off_smooth_60_in_prefs(v);
        return true;
    }
    let Some(p) = resolve_vs_script_path(v) else {
        eprintln!(
            "[rhino] video: VapourSynth: no .vpy (install mvtools + data/vs bundle; see `data/vs/README.md`)."
        );
        turn_off_smooth_60_in_prefs(v);
        return true;
    };
    eprintln!("[rhino] video: VapourSynth script = {p}");
    let bf = SMOOTH_VF_BUFFERED_FRAMES;
    let spec = format!(
        "vapoursynth:file={}:buffered-frames={bf}:concurrent-frames=auto",
        mpv_escape_path(&p),
    );
    if let Err(e) = mpv.command("vf", &["add", &spec]) {
        eprintln!("[rhino] video: vf add vapoursynth failed: {e:?} (trying set_property; install VapourSynth + mvtools if this persists).");
        if let Err(e2) = mpv.set_property("vf", spec.clone()) {
            eprintln!("[rhino] video: set_property vf fallback failed: {e2:?}");
            turn_off_smooth_60_in_prefs(v);
            return true;
        }
        eprintln!("[rhino] video: VapourSynth set via `vf` property (fallback after vf add error)");
    } else {
        eprintln!("[rhino] video: vf add vapoursynth command accepted");
    }
    apply_smooth_vf_present_opts(mpv);
    false
}

