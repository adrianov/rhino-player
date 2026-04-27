use std::path::Path;

use libmpv2::Mpv;

use crate::db;
use crate::db::VideoPrefs;
use crate::paths;
use crate::paths::RHINO_PLAYBACK_SPEED_VAR;

/// [apply_mpv_video] result (replaces a bare `bool` for "smooth was auto-off" on older call sites).
#[derive(Debug)]
pub struct MpvVideoApply {
    /// Prefs had **Smooth 60** turned off (missing script, `vf` rejected, etc.).
    pub smooth_auto_off: bool,
}

fn video_log() -> bool {
    std::env::var("RHINO_VIDEO_LOG")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Stores a stable absolute path for SQLite ([VideoPrefs::mvtools_lib]).
fn mvt_path_to_store(p: &Path) -> String {
    p.canonicalize()
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

/// Resolves `libmvtools.so`, sets `RHINO_MVTOOLS_LIB` (in-process mpv inherits the environment).
/// Order: env [paths::mvtools_from_env], then **cached** [VideoPrefs::mvtools_lib] if still a file, else
/// [paths::mvtools_lib_search]; on success, saves the full path in settings so the scan is not repeated
/// while the file exists. Returns `false` when `libmvtools.so` cannot be resolved.
fn apply_mvtools_env(v: &mut VideoPrefs) -> bool {
    if let Some(p) = paths::mvtools_from_env() {
        let s = mvt_path_to_store(&p);
        if v.mvtools_lib != s {
            v.mvtools_lib = s;
            db::save_video(v);
        }
        std::env::set_var(paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
        eprintln!(
            "[rhino] video: libmvtools -> {} (from {})",
            v.mvtools_lib,
            paths::RHINO_MVTOOLS_LIB_VAR
        );
        return true;
    }
    let c = v.mvtools_lib.trim();
    if !c.is_empty() {
        if Path::new(c).is_file() {
            std::env::set_var(paths::RHINO_MVTOOLS_LIB_VAR, c);
            eprintln!("[rhino] video: libmvtools -> {c} (cached in settings)");
            return true;
        }
        v.mvtools_lib.clear();
        db::save_video(v);
    }
    if let Some(p) = paths::mvtools_lib_search() {
        v.mvtools_lib = mvt_path_to_store(&p);
        db::save_video(v);
        std::env::set_var(paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
        eprintln!("[rhino] video: libmvtools -> {}", v.mvtools_lib);
        true
    } else {
        eprintln!(
            "[rhino] video: libmvtools.so not found; set {} or install MVTools with vsrepo (see `data/vs/README.md`).",
            paths::RHINO_MVTOOLS_LIB_VAR
        );
        false
    }
}

/// “≈1.0×” band: bundled mvtools [vf] eligibility and env comparison use this tolerance.
const PLAYBACK_1X_EPS: f64 = 0.001;
/// VapourSynth / mvtools needs a deeper queue than ordinary decode to avoid jitter from CPU spikes.
const VS_BUFFERED_FRAMES: i32 = 24;

/// Same string [mpv] and the VapourSynth script use for `RHINO_PLAYBACK_SPEED`.
fn normalized_env_speed(s: f64) -> f64 {
    if !s.is_finite() {
        return 1.0;
    }
    let s = if s > 0.01 && s < 8.0 { s } else { 1.0 };
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
fn mvtools_vf_eligible(mpv: &Mpv, speed_hint: Option<f64>) -> bool {
    let s = match speed_hint {
        Some(x) if x.is_finite() => normalized_env_speed(x),
        _ => match mpv.get_property::<f64>("speed") {
            Ok(v) if v.is_finite() => normalized_env_speed(v),
            _ => 1.0,
        },
    };
    (s - 1.0).abs() <= PLAYBACK_1X_EPS
}

/// `true` when the process env disagrees with current [mpv] `speed` (e.g. [vf] added before watch-later
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
/// only at ~1.0×; strip when sped up). Returns [MpvVideoApply::smooth_auto_off].
pub fn resync_smooth_if_speed_mismatch(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return false;
    }
    let want_mvtools = mvtools_vf_eligible(mpv, None);
    let has = vf_string_has_vapoursynth(mpv);
    if !needs_playback_speed_env_resync(mpv) && want_mvtools == has {
        return false;
    }
    apply_mpv_video(mpv, v, None).smooth_auto_off
}

/// After [libmpv2::Mpv] `speed` changes: re-run [apply_mpv_video] so `vf` / decode match
/// (mvtools only at ~1.0×; see [mvtools_vf_eligible]).
/// Pass [speed_hint] with the `speed` you just set in mpv to avoid a **get_property** race; use `None` to
/// read the current [mpv] value.
/// Returns `true` if **Smooth 60** was auto-disabled in prefs.
pub fn refresh_smooth_for_playback_speed(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> bool {
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return false;
    }
    eprintln!("[rhino] video: video pipeline resync for playback speed");
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    apply_mpv_video(mpv, v, speed_hint).smooth_auto_off
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
fn mpv_has_open_media(mpv: &Mpv) -> bool {
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
    let spec = format!(
        "vapoursynth:file={}:buffered-frames={VS_BUFFERED_FRAMES}:concurrent-frames=auto",
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
    false
}

