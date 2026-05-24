use std::path::Path;
use std::sync::atomic::AtomicU64;

use crate::db;
use crate::db::VideoPrefs;
use crate::paths;
use crate::paths::{
    publish_smooth_me_budget_env, smooth_max_area_env_matches, RHINO_PLAYBACK_SPEED_VAR,
    RHINO_SMOOTH_MAX_AREA_VAR, RHINO_VPY_LOG_EPOCH_VAR,
};
use crate::playback_speed::MAX_FIXED_SPEED;

include!("mvtools_vf_substring_checks.rs");

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
/// Bundled/custom VapourSynth graph — skipped when interleaved cadence needs display-resample only.
pub(crate) fn smooth_wants_vapoursynth_vf(
    mpv: &Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    speed_hint: Option<f64>,
) -> bool {
    mvtools_vf_eligible(mpv, speed_hint)
        && !smooth_prefers_display_resample_bundle(mpv, bundle)
}

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
    let want_mvtools = smooth_wants_vapoursynth_vf(mpv, Some(b), None);
    let has = vf_chain_has_vapoursynth(mpv);
    let graph_ok = !has || vf_smooth_matches_prefs(mpv, v, Some(b));
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
    if !vfl.contains("vapoursynth") {
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
    let want_deint = wants_bluray_bob_deinterlace(mpv, bundle);
    if want_deint && !bluray_deinterlace_in_vf(&vf) {
        return false;
    }
    if !vf_smooth_queue_chain_ok(&vf) {
        return false;
    }
    if !vf_concurrent_frames_matches(&vf, "auto") {
        return false;
    }
    let me_cap = effective_smooth_me_budget_px(mpv, v, bundle);
    if v.vs_path.trim().is_empty() && !smooth_max_area_env_matches(me_cap) {
        return false;
    }
    bundled_me_budget_vf_matches_prefs(mpv, v, bundle)
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

fn add_smooth_60(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    cadence_hz: Option<f64>,
) -> bool {
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
    if !smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint) {
        return false;
    }
    ensure_hwdec_vf_copy(mpv);
    sync_bluray_deinterlace_mpv(mpv, bundle);
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    let cap_px = effective_smooth_me_budget_px(mpv, v, bundle);
    let fps_opt = cadence_hz.or_else(|| refresh_smooth_cadence_gate(mpv, bundle));
    if v.vs_path.trim().is_empty() {
        publish_smooth_me_budget_env(cap_px);
        if video_log() {
            eprintln!(
                "[rhino] video: (verbose) bundled ME px²={cap_px} ({})",
                RHINO_SMOOTH_MAX_AREA_VAR
            );
        }
    }
    apply_source_fps_env(fps_opt);
    if video_log() {
        eprintln!(
            "[rhino] video: (verbose) vapoursynth buffered-frames={}",
            SMOOTH_VF_BUFFERED_FRAMES
        );
    }
    let epoch = VPY_LOG_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    let p_esc = mpv_escape_path(&p);
    let me_cap = cap_px;
    if !smooth_vapoursynth_vf_try_attach(mpv, &p_esc) {
        turn_off_smooth_60_in_prefs(v);
        return true;
    }
    if v.vs_path.trim().is_empty() {
        let media_key = me_budget_local_path(mpv, bundle)
            .as_ref()
            .and_then(|p| crate::db::history_key(p.as_path()));
        note_bundled_me_budget_vf_applied(me_cap, media_key);
    }
    apply_smooth_vf_present_opts(mpv);
    smooth_on_refresh_playhead(mpv, bundle);
    false
}
