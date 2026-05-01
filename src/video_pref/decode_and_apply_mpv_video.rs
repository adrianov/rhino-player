use std::time::Instant;

use crate::mpv_embed::MpvBundle;

/// Wall-clock wait after playback starts before attaching the Smooth 60 `vf` (see
/// [apply_mpv_fast_start_after_load] + [crate::app::schedule_smooth_vf_attach_after_delay]).
pub const SMOOTH_VF_ATTACH_DELAY_MS: u64 = 500;

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

fn smooth_vf_delay_active(b: &MpvBundle) -> bool {
    b.smooth_vf_not_before
        .get()
        .is_some_and(|t| Instant::now() < t)
}

/// After [loadfile] with Smooth 60 at ~1.0×: **no** VapourSynth `vf` yet, **`hwdec=auto`** so the first
/// frames come up quickly, then [crate::app::schedule_smooth_vf_attach_after_delay] attaches the script
/// after [SMOOTH_VF_ATTACH_DELAY_MS] if still playing. Other cases delegate to [apply_mpv_video].
pub fn apply_mpv_fast_start_after_load(b: &MpvBundle, v: &mut VideoPrefs) -> MpvVideoApply {
    let mpv = &b.mpv;
    let use_mvtools_now = v.smooth_60 && mvtools_vf_eligible(mpv, None);
    if !use_mvtools_now || !mpv_has_open_media(mpv) {
        return apply_mpv_video(b, v, None);
    }
    let vlog = video_log();
    if vf_string_has_vapoursynth(mpv) {
        clear_vf(mpv, vlog);
    }
    set_auto_decode(mpv, vlog);
    if video_log() {
        eprintln!("[rhino] video: after load — decode without Smooth `vf`; attach if still playing after {} ms", SMOOTH_VF_ATTACH_DELAY_MS);
    }
    log_vf_diagnostics(mpv, vlog);
    MpvVideoApply::default()
}

/// If Smooth 60 is on, **speed** is ~1.0×, and `vapoursynth` is still not in the `vf` list, run
/// [apply_mpv_video] once (e.g. after the post-load attach timer).
pub fn reapply_60_if_still_missing(b: &MpvBundle, v: &mut VideoPrefs) -> MpvVideoApply {
    let mpv = &b.mpv;
    if mpv.get_property::<bool>("pause").unwrap_or(true) {
        return MpvVideoApply::default();
    }
    if !v.smooth_60 || !mpv_has_open_media(mpv) {
        return MpvVideoApply::default();
    }
    if !mvtools_vf_eligible(mpv, None) {
        return MpvVideoApply::default();
    }
    if vf_string_has_vapoursynth(mpv) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: reapply_60_if_still_missing → apply_mpv_video");
    apply_mpv_video(b, v, None)
}

pub(crate) fn vf_chain_has_vapoursynth(mpv: &Mpv) -> bool {
    vf_string_has_vapoursynth(mpv)
}

fn vf_string_has_vapoursynth(mpv: &Mpv) -> bool {
    match mpv.get_property::<String>("vf") {
        Ok(s) => s.to_lowercase().contains("vapoursynth"),
        Err(_) => false,
    }
}

/// Drop the Smooth / VapourSynth `vf` while **paused** so the player shows a normal still frame;
/// restore with [apply_mpv_video] when playback resumes (transport `pause` handling).
pub fn unload_smooth_on_pause(mpv: &Mpv) -> bool {
    if !vf_string_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, vlog);
    set_auto_decode(mpv, vlog);
    true
}
pub fn apply_mpv_video_init(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    apply_mpv_video_impl(mpv, v, None, None)
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
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth Video (~60 FPS at 1.0×)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }
}

pub fn apply_mpv_video(b: &MpvBundle, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, v, speed_hint, Some(&b.smooth_vf_not_before))
}

fn apply_mpv_video_impl(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    clear_smooth_delay: Option<&std::cell::Cell<Option<Instant>>>,
) -> MpvVideoApply {
    if let Some(c) = clear_smooth_delay {
        c.set(None);
    }
    let vlog = video_log();
    log_apply(v);
    let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
    let use_mvtools = v.smooth_60 && mvtools_vf_eligible(mpv, speed_hint) && !paused;
    let want_60 = v.smooth_60;
    let had_vapoursynth = vf_string_has_vapoursynth(mpv);
    if !use_mvtools {
        if had_vapoursynth {
            clear_vf(mpv, vlog);
            set_auto_decode(mpv, vlog);
        }
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply::default();
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    // Leave hwdec / vd-lavc-dr unchanged when attaching VS (default hwdec=auto is fine on typical stacks).
    clear_vf(mpv, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
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
