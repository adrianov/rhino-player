//! mpv `video-sync` / `vf` from [crate::db::VideoPrefs].
//! See `docs/features/26-sixty-fps-motion.md`.
//! Set `RHINO_VIDEO_LOG=1` for per-step mpv result lines on stderr.
//!
//! If the VapourSynth `vf` cannot be added (no script, or mpv reports error — missing filter, plugin,
//! Python), [apply_mpv_video] sets `smooth_60` to `false`, saves settings, and returns `true` so the UI
//! can sync the **Smooth video (60 FPS)** menu.
//!
//! **Hardware decode** (`hwdec=auto` / VAAPI / NVDEC) often **bypasses** the CPU VapourSynth path, so
//! the filter is inert and motion looks identical to 24p. We set **`hwdec=no`** while 60p is on, and
//! restore **`hwdec=auto`** when it is off or the vf is rejected.

use std::path::Path;

use libmpv2::Mpv;

use crate::db;
use crate::db::VideoPrefs;
use crate::paths;

fn video_log() -> bool {
    std::env::var("RHINO_VIDEO_LOG")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// User or bundled `.vpy` for `vapoursynth` when [VideoPrefs::smooth_60] is set.
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
    eprintln!("[rhino] video: VapourSynth script = {p}");
    let spec = format!(
        "vapoursynth:file={}:buffered-frames=8:concurrent-frames=auto",
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

/// Fixed timing: `video-sync=audio`, no `display-resample` / `interpolation`. Replaces the whole `vf`
/// list (clears previous), then optional VapourSynth per prefs.
/// Returns `true` if **smooth 60** was auto-disabled (prefs saved) so the app can uncheck the menu.
pub fn apply_mpv_video(mpv: &Mpv, v: &mut VideoPrefs) -> bool {
    let vlog = video_log();
    eprintln!(
        "[rhino] video: apply_mpv_video smooth_60={} vs_path_len={}",
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
            eprintln!("[rhino] video: hwdec=no (VapourSynth needs CPU frames; see docs/features/26-sixty-fps-motion.md)");
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

    let disabled_60 = add_smooth_60(mpv, v);
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
    disabled_60
}

fn mpv_escape_path(p: &str) -> String {
    if p.contains(':') || p.contains(' ') || p.contains('[') {
        format!("[{p}]")
    } else {
        p.to_string()
    }
}
