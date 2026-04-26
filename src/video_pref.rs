//! mpv `video-sync` / `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`.
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth video (60 FPS)** menu.
//! If `vf` **add** succeeds but mpv **disables** the filter at runtime (e.g. script error, missing
//! `mvtools` / `core.mv`), [tick_reconcile_failed_vapoursynth] detects a missing `vapoursynth` entry in
//! the `vf` list and applies the same auto-off + [apply_mpv_video] restore.
//!
//! **Hardware decode** (`hwdec=auto` / VAAPI / NVDEC) often **bypasses** the CPU VapourSynth path, so
//! the filter is inert and motion looks identical to 24p. We set **`hwdec=no`** while 60p is on, and
//! restore **`hwdec=auto`** when it is off or the vf is rejected.
//! Successful `libmvtools.so` resolution is stored in SQLite (`video_mvtools_lib`); the next session
//! reuses that path if the file still exists, avoiding a full search.
//!
//! On [loadfile], [ApplyMpvVideoMode::DeferVapourSynth] clears the `vf` chain and sets decode options
//! **without** attaching VapourSynth immediately, so a few **plain** software-decoded frames can show
//! first; the app then calls [complete_vapoursynth_attach] after [VS_DEFER_MS].

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use libmpv2::Mpv;

use crate::db;
use crate::db::VideoPrefs;
use crate::paths;

/// Milliseconds after [loadfile] before attaching the VapourSynth `vf` when using [ApplyMpvVideoMode::DeferVapourSynth].
pub const VS_DEFER_MS: u64 = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApplyMpvVideoMode {
    /// Clear `vf` and attach VapourSynth in the same call (toggles, menu, init without media).
    #[default]
    Full,
    /// After [loadfile] with Smooth 60: prepare decode path, attach VapourSynth in [complete_vapoursynth_attach].
    DeferVapourSynth,
}

/// [apply_mpv_video] result (replaces a bare `bool` for "smooth was auto-off" on older call sites).
#[derive(Debug)]
pub struct MpvVideoApply {
    /// Prefs had **Smooth 60** turned off (missing script, `vf` rejected, etc.).
    pub smooth_auto_off: bool,
    /// VapourSynth not attached yet; call [complete_vapoursynth_attach] after [VS_DEFER_MS].
    pub vapoursynth_deferred: bool,
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
/// while the file exists.
fn apply_mvtools_env(v: &mut VideoPrefs) {
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
        return;
    }
    let c = v.mvtools_lib.trim();
    if !c.is_empty() {
        if Path::new(c).is_file() {
            std::env::set_var(paths::RHINO_MVTOOLS_LIB_VAR, c);
            eprintln!("[rhino] video: libmvtools -> {c} (cached in settings)");
            return;
        }
        v.mvtools_lib.clear();
        db::save_video(v);
    }
    if let Some(p) = paths::mvtools_lib_search() {
        v.mvtools_lib = mvt_path_to_store(&p);
        db::save_video(v);
        std::env::set_var(paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
        eprintln!("[rhino] video: libmvtools -> {}", v.mvtools_lib);
    } else {
        eprintln!(
            "[rhino] video: libmvtools.so not found; set {} or install vapoursynth-mvtools (VapourSynth may still autoload the plugin; see `data/vs/README.md`).",
            paths::RHINO_MVTOOLS_LIB_VAR
        );
    }
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

/// Called from the main UI ~200ms poll. When the user has **Smooth 60** on, mpv may accept `vf add`
/// then **remove** the VapourSynth filter when the script fails (e.g. `AttributeError: No attribute mv`).
/// In that case `vf` no longer contains `vapoursynth` while prefs still have `smooth_60` and `hwdec=no`.
///
/// We require `duration` and `time-pos` past a short start window, then require **3** consecutive
/// missing checks (~600ms) to avoid racing the idle/600ms [apply_mpv_video] reapply.
///
/// Returns `true` if prefs were updated and the caller should uncheck `smooth-60` in the menu.
pub fn tick_reconcile_failed_vapoursynth(
    mpv: &Mpv,
    vp: &Rc<RefCell<VideoPrefs>>,
    gone_ticks: &Cell<u8>,
    pos: f64,
    dur: f64,
) -> bool {
    if !vp.borrow().smooth_60 {
        gone_ticks.set(0);
        return false;
    }
    if !mpv_has_open_media(mpv) {
        return false;
    }
    if dur < 0.1 {
        return false;
    }
    // Defer until frames can have reached the filter (also avoids racing post-load reapply).
    if pos < 0.25 {
        gone_ticks.set(0);
        return false;
    }
    let vs_ok = match mpv.get_property::<String>("vf") {
        Ok(s) => s.to_lowercase().contains("vapoursynth"),
        Err(_) => false,
    };
    if vs_ok {
        gone_ticks.set(0);
        return false;
    }
    let n = gone_ticks.get().saturating_add(1);
    gone_ticks.set(n);
    if n < 3 {
        return false;
    }
    eprintln!(
        "[rhino] video: VapourSynth was dropped from `vf` at runtime (script/plugin error; install mvtools — see `data/vs/README.md`) — saved `video_smooth_60` = 0."
    );
    let mut g = vp.borrow_mut();
    turn_off_smooth_60_in_prefs(&mut g);
    let _ = apply_mpv_video(mpv, &mut g, ApplyMpvVideoMode::Full);
    gone_ticks.set(0);
    true
}

/// After `vf` is cleared, add ~60 fps filter when [VideoPrefs::smooth_60]. Returns `true` if we
/// **disabled** the option in prefs (VapourSynth path missing and no bundle, or `vf` add failed).
/// True when a media file is open (filters must attach after [loadfile] so `video_in` exists).
fn mpv_has_open_media(mpv: &Mpv) -> bool {
    // `path` is the main/selected file; empty before the first `loadfile` or while idle.
    match mpv.get_property::<String>("path") {
        Ok(s) if !s.trim().is_empty() => true,
        _ => false,
    }
}

fn add_smooth_60(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    if !v.smooth_60 {
        return false;
    }
    if !mpv_has_open_media(mpv) {
        // Init-time [apply_mpv_video] and pre-load calls must *not* run `vf add` (no `video_in` / no
        // `path` yet) — a failed add used to look like a broken install and **disabled 60p in the DB**.
        eprintln!("[rhino] video: VapourSynth deferred (no `path` yet — will apply after loadfile)");
        return false;
    }
    let Some(p) = resolve_vs_script_path(v) else {
        eprintln!(
            "[rhino] video: VapourSynth: no .vpy (install mvtools + data/vs bundle; see `data/vs/README.md`)."
        );
        turn_off_smooth_60_in_prefs(v);
        return true;
    };
    apply_mvtools_env(v);
    eprintln!("[rhino] video: VapourSynth script = {p}");
    let spec = format!(
        "vapoursynth:file={}:buffered-frames=3:concurrent-frames=auto",
        mpv_escape_path(&p)
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

fn log_vf_diagnostics(mpv: &Mpv, vlog: bool) {
    match mpv.get_property::<String>("vf") {
        Ok(s) if !s.is_empty() => eprintln!("[rhino] video: mpv property `vf` = {s:?}"),
        Ok(_) => eprintln!("[rhino] video: mpv property `vf` is empty (no file, or not applied yet)"),
        Err(e) => eprintln!("[rhino] video: could not read mpv property `vf`: {e:?}"),
    }
    if vlog {
        if let Ok(s) = mpv.get_property::<String>("video-sync") {
            eprintln!("[rhino] video: (verbose) video-sync = {s:?}");
        }
    }
}

/// Second step after [ApplyMpvVideoMode::DeferVapourSynth]: [add_smooth_60] + restore decode prefs if
/// Smooth 60 was auto-off.
pub fn complete_vapoursynth_attach(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    let want_60 = v.smooth_60;
    let vlog = video_log();
    eprintln!("[rhino] video: complete_vapoursynth_attach");
    let disabled_60 = add_smooth_60(mpv, v);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    disabled_60
}

/// If Smooth 60 is on but `vapoursynth` is still not in the `vf` list (e.g. deferred attach not run yet
/// or a race), [add_smooth_60] only — no full [vf] clear (avoids a second black window).
pub fn reapply_60_if_still_missing(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
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

/// Fixed timing: `video-sync=audio`, no `display-resample` / `interpolation`. Replaces the whole `vf`
/// list (clears previous), then optional VapourSynth per prefs (or defers that step — see [ApplyMpvVideoMode]).
pub fn apply_mpv_video(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    mode: ApplyMpvVideoMode,
) -> MpvVideoApply {
    let vlog = video_log();
    eprintln!(
        "[rhino] video: apply_mpv_video mode={mode:?} smooth_60={} vs_path_len={}",
        v.smooth_60,
        v.vs_path.len()
    );
    if !v.smooth_60 {
        eprintln!(
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth video (60 FPS)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }

    let want_60 = v.smooth_60;
    if want_60 {
        if let Err(e) = mpv.set_property("hwdec", "no") {
            eprintln!("[rhino] video: set hwdec no failed: {e:?}");
        } else {
            eprintln!("[rhino] video: hwdec=no (vapoursynth vf: software decode so the filter path runs; hwdec=auto often skips it — see docs/features/26-sixty-fps-motion.md)");
        }
        // Direct rendering can avoid feeding software filters (same family of issues as hwdec).
        if let Err(e) = mpv.set_property("vd-lavc-dr", "no") {
            eprintln!("[rhino] video: set vd-lavc-dr no failed: {e:?}");
        } else if vlog {
            eprintln!("[rhino] video: vd-lavc-dr=no (with smooth 60)");
        }
    } else if let Err(e) = mpv.set_property("hwdec", "auto") {
        eprintln!("[rhino] video: set hwdec auto failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: hwdec=auto (smooth 60 off)");
    }
    if !want_60 {
        let _ = mpv.set_property("vd-lavc-dr", "auto");
    }

    if let Err(e) = mpv.set_property("video-sync", "audio") {
        eprintln!("[rhino] video: set video-sync audio failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: set video-sync -> audio ok");
    }
    if let Err(e) = mpv.set_property("interpolation", false) {
        eprintln!("[rhino] video: set interpolation false failed: {e:?}");
    } else if vlog {
        eprintln!("[rhino] video: set interpolation false ok");
    }

    // Always clear via `vf clr ""` (mpv requires a second arg for clr). Relying only on
    // `set_property vf` can leave VapourSynth running after toggling off.
    if let Err(e) = mpv.command("vf", &["clr", ""]) {
        eprintln!("[rhino] video: vf clr failed: {e:?}; trying set_property vf");
        if let Err(e2) = mpv.set_property("vf", "") {
            eprintln!("[rhino] video: set_property vf (clear) failed: {e2:?}");
        }
    } else if vlog {
        eprintln!("[rhino] video: vf clr ok");
    }
    let _ = mpv.set_property("vf", "");

    let defer = mode == ApplyMpvVideoMode::DeferVapourSynth
        && want_60
        && mpv_has_open_media(mpv)
        && resolve_vs_script_path(v).is_some();

    if defer {
        eprintln!(
            "[rhino] video: VapourSynth attach delayed {VS_DEFER_MS}ms (plain software-decoded frames first; then complete_vapoursynth_attach)"
        );
        log_vf_diagnostics(mpv, vlog);
        return MpvVideoApply {
            smooth_auto_off: false,
            vapoursynth_deferred: true,
        };
    }

    let disabled_60 = add_smooth_60(mpv, v);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
        vapoursynth_deferred: false,
    }
}

fn mpv_escape_path(p: &str) -> String {
    if p.contains(':') || p.contains(' ') || p.contains('[') {
        format!("[{p}]")
    } else {
        p.to_string()
    }
}
