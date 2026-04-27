//! Optional mpv VapourSynth `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`. Sets [crate::paths::RHINO_PLAYBACK_SPEED_VAR] from mpv
//! `speed` before the VapourSynth filter is built. The graph is **rebuilt on events**: after [loadfile]
//! (idle + follow-up idle), when the user picks **playback speed** in the header (deferred idle), and on
//! the **on-file-loaded** hook (one-shot) so **watch-later** restored `speed` / UI list align with the [vf]
//! and env. There is **no** periodic “watch” on `vf` for runtime plugin failures — `vf` add failure still
//! clears the pref at apply time; a script that dies *after* add is a rare install issue (toggle off in
//! **Preferences** or fix mvtools).
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth video (~60 FPS at 1.0×)** menu.
//!
//! **Hardware decode** (`hwdec=auto` / VAAPI / NVDEC) often **bypasses** the CPU VapourSynth path, so
//! the filter is inert and motion looks identical to 24p. Normal playback leaves mpv defaults alone.
//! We set **`hwdec=no`** only while adding the mvtools [vf] (Smooth 60 and **1.0×**), and restore
//! **`hwdec=auto`** only when removing a previously active VapourSynth graph.
//! Successful `libmvtools.so` resolution is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! [try_load] runs [apply_mpv_video] on the next idle after [loadfile] so the `vapoursynth` [vf] is
//! installed in the same pass as the rest of the mpv state. After a **vf** clear/replace while playing,
//! we **re-seek** to the current [time-pos] so the video track realigns to audio (toggling filters or
//! sw/hw decode can leave mpv A/V offset until a seek). A **brief** black may appear while VapourSynth
//! warms up.

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

fn post_smooth_60_state(mpv: &Mpv, v: &VideoPrefs, want_60: bool, disabled_60: bool, vlog: bool) {
    if want_60 && !v.smooth_60 {
        if let Err(e) = mpv.set_property("hwdec", "auto") {
            eprintln!("[rhino] video: set hwdec auto after VapourSynth off: {e:?}");
        } else {
            eprintln!("[rhino] video: hwdec=auto (VapourSynth path missing or vf rejected)");
        }
        let _ = mpv.set_property("vd-lavc-dr", "auto");
    }
    if disabled_60 {
        eprintln!("[rhino] video: saved `video_smooth_60` = 0 (VapourSynth path unusable or vf rejected).");
    }
    log_vf_diagnostics(mpv, vlog);
}

fn set_smooth_decode(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.set_property("hwdec", "no") {
        eprintln!("[rhino] video: set hwdec no failed: {e:?}");
    } else {
        eprintln!("[rhino] video: hwdec=no (vapoursynth vf: software decode so the filter path runs; hwdec=auto often skips it — see docs/features/26-sixty-fps-motion.md)");
    }
    if let Err(e) = mpv.set_property("vd-lavc-dr", "no") {
        eprintln!("[rhino] video: set vd-lavc-dr no failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: vd-lavc-dr=no (with smooth 60 at 1.0×)");
    }
}

fn set_auto_decode(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.set_property("hwdec", "auto") {
        eprintln!("[rhino] video: set hwdec auto failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: hwdec=auto (no mvtools vf: smooth off or speed ≠ 1.0×)");
    }
    let _ = mpv.set_property("vd-lavc-dr", "auto");
}

fn clear_vf(mpv: &Mpv, vlog: bool) {
    if let Err(e) = mpv.command("vf", &["clr", ""]) {
        eprintln!("[rhino] video: vf clr failed: {e:?}; trying set_property vf");
        if let Err(e2) = mpv.set_property("vf", "") {
            eprintln!("[rhino] video: set_property vf (clear) failed: {e2:?}");
        }
    } else if vlog {
        eprintln!("[rhino] video: vf clr ok");
    }
    let _ = mpv.set_property("vf", "");
}

fn log_vf_diagnostics(mpv: &Mpv, vlog: bool) {
    match mpv.get_property::<String>("vf") {
        Ok(s) if !s.is_empty() => eprintln!("[rhino] video: mpv property `vf` = {s:?}"),
        Ok(_) => {
            eprintln!("[rhino] video: mpv property `vf` is empty (no file, or not applied yet)")
        }
        Err(e) => eprintln!("[rhino] video: could not read mpv property `vf`: {e:?}"),
    }
    if vlog {
        if let Ok(s) = mpv.get_property::<String>("video-sync") {
            eprintln!("[rhino] video: (verbose) video-sync = {s:?}");
        }
    }
}

/// [apply_mpv_video] when the VapourSynth [vf] was not installed yet; see [mvtools_vf_eligible] for when
/// the filter is actually added.
pub fn complete_vapoursynth_attach(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    eprintln!("[rhino] video: complete_vapoursynth_attach");
    apply_mpv_video(mpv, v, None).smooth_auto_off
}

/// If Smooth 60 is on, **speed** is ~1.0×, and `vapoursynth` is still not in the `vf` list (e.g. post-load
/// race), run [apply_mpv_video] once. Called from the **second** [loadfile] idle (chained), not from a timer.
pub fn reapply_60_if_still_missing(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return false;
    }
    if !mvtools_vf_eligible(mpv, None) {
        return false;
    }
    if vf_string_has_vapoursynth(mpv) {
        return false;
    }
    complete_vapoursynth_attach(mpv, v)
}

fn vf_string_has_vapoursynth(mpv: &Mpv) -> bool {
    match mpv.get_property::<String>("vf") {
        Ok(s) => s.to_lowercase().contains("vapoursynth"),
        Err(_) => false,
    }
}

/// True when the active mpv video filter list contains VapourSynth.
pub fn has_vapoursynth_vf(mpv: &Mpv) -> bool {
    vf_string_has_vapoursynth(mpv)
}

/// Clear VapourSynth only for paused seeking, so mpv can show a still frame without a black GL surface.
pub fn clear_vapoursynth_for_paused_seek(mpv: &Mpv) -> bool {
    if !vf_string_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, vlog);
    set_auto_decode(mpv, vlog);
    true
}

/// After the video filter list or decode path changes, re-align the video track to the audio clock
/// by [seek]ing to the current position (libmpv, same as input.conf). Skips at file start to avoid
/// fighting [try_load], and with zero/invalid duration.
fn resync_av_after_vf_change(mpv: &Mpv) {
    if !mpv_has_open_media(mpv) {
        return;
    }
    let dur = mpv.get_property::<f64>("duration").unwrap_or(0.0);
    if !dur.is_finite() || dur <= 0.0 {
        return;
    }
    let pos = match mpv.get_property::<f64>("time-pos") {
        Ok(p) if p.is_finite() && p >= 0.0 => p,
        _ => return,
    };
    if pos < 0.12 {
        return;
    }
    let end = (dur - 0.05).max(0.0);
    let t = pos.clamp(0.0, end);
    let s = format!("{:.4}", t);
    match mpv.command("seek", &[s.as_str(), "absolute+keyframes"]) {
        Ok(()) => {
            if video_log() {
                eprintln!("[rhino] video: A/V resync after vf (seek) @ {t:.3}s");
            }
        }
        Err(e) => {
            eprintln!("[rhino] video: seek resync after vf failed: {e:?}; trying time-pos");
            let _ = mpv.set_property("time-pos", t);
        }
    }
}

/// Normal playback is intentionally a no-op: leave mpv's timing, decode, and filter defaults alone.
/// When Smooth 60 is active, replace the `vf` list and add VapourSynth at ~**1.0×** only.
/// [speed_hint] is passed to [add_smooth_60] when set (e.g. header row) to match env before the [vf] add.
fn log_apply(v: &VideoPrefs) {
    eprintln!(
        "[rhino] video: apply_mpv_video smooth_60={} vs_path_len={}",
        v.smooth_60,
        v.vs_path.len()
    );
    if !v.smooth_60 {
        eprintln!(
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth video (~60 FPS at 1.0×)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }
}

pub fn apply_mpv_video(mpv: &Mpv, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    let vlog = video_log();
    log_apply(v);
    let use_mvtools = v.smooth_60 && mvtools_vf_eligible(mpv, speed_hint);
    let want_60 = v.smooth_60;
    let had_vapoursynth = vf_string_has_vapoursynth(mpv);
    if !use_mvtools {
        if had_vapoursynth {
            clear_vf(mpv, vlog);
            set_auto_decode(mpv, vlog);
            resync_av_after_vf_change(mpv);
        }
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply {
            smooth_auto_off: false,
        };
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    set_smooth_decode(mpv, vlog);
    clear_vf(mpv, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    if !disabled_60 {
        resync_av_after_vf_change(mpv);
    }
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}

fn mpv_escape_path(p: &str) -> String {
    if p.contains(':') || p.contains(' ') || p.contains('[') {
        format!("[{p}]")
    } else {
        p.to_string()
    }
}

#[cfg(test)]
mod model_tests {
    //! [super::mvtools_vf_eligible] is the source of truth; this module mirrors the **speed** part so
    //! tests do not need an [Mpv] handle.

    use super::normalized_env_speed;
    use super::PLAYBACK_1X_EPS;

    fn mvtools_vf_wanted_for_speed(s: f64) -> bool {
        let t = normalized_env_speed(s);
        (t - 1.0).abs() <= PLAYBACK_1X_EPS
    }

    /// When the graph **should** include `vapoursynth` (pref on + ~1.0×) but the string does not, an
    /// [apply_mpv_video] (or [super::reapply_60_if_still_missing] after load) is the way to fix it — not a timer.
    fn graph_lacks_script_while_wanted(
        smooth_pref: bool,
        playback_speed: f64,
        vf_has_vapoursynth: bool,
    ) -> bool {
        smooth_pref && mvtools_vf_wanted_for_speed(playback_speed) && !vf_has_vapoursynth
    }

    #[test]
    fn bundled_script_only_at_1x() {
        assert!(mvtools_vf_wanted_for_speed(1.0));
        assert!(!mvtools_vf_wanted_for_speed(1.5));
        assert!(!mvtools_vf_wanted_for_speed(2.0));
    }

    #[test]
    fn sped_up_does_not_require_vapoursynth_in_vf() {
        assert!(!graph_lacks_script_while_wanted(true, 1.5, false));
        assert!(!graph_lacks_script_while_wanted(true, 2.0, false));
    }

    #[test]
    fn at_1x_pref_on_missing_vf_is_stale_graph() {
        assert!(graph_lacks_script_while_wanted(true, 1.0, false));
        assert!(!graph_lacks_script_while_wanted(true, 1.0, true));
        assert!(!graph_lacks_script_while_wanted(false, 1.0, false));
    }
}
