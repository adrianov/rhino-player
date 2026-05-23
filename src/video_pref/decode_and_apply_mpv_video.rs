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
    let need_hwdec = mpv
        .get_property::<String>("hwdec")
        .map(|s| s != "auto")
        .unwrap_or(true);
    if need_hwdec {
        if let Err(e) = mpv.set_property("hwdec", "auto") {
            eprintln!("[rhino] video: set hwdec auto failed: {e:?}");
        } else if vlog {
            eprintln!("[rhino] video: hwdec=auto (no mvtools vf: smooth off or speed ≠ 1.0×)");
        }
    }
    let need_dr = mpv
        .get_property::<String>("vd-lavc-dr")
        .map(|s| s != "auto")
        .unwrap_or(true);
    if need_dr {
        let _ = mpv.set_property("vd-lavc-dr", "auto");
    }
}

/// Plain playback after Smooth **off** / **`vf`** strip — prefer **`clear_vf`** which ends with this once **`vf`** is empty.
///
/// **`vo=libmpv`**: **`display-resample`** + **`report_swap`** (Linux **EGL** / **GLArea** + macOS **CVDisplayLink**).
/// Fallback **`audio`** + swap gate off if **`display-resample`** fails.
/// **`vf clr`** runs inside **`with_macos_vf_teardown`** when a bundle is passed (macOS).
fn restore_non_smooth_present_opts(mpv: &Mpv) {
    let _ = mpv.set_property("interpolation", "no");
    if mpv.set_property("video-sync", "display-resample").is_ok() {
        smooth_vf_swap_timing_set(true);
    } else {
        let _ = mpv.set_property("video-sync", "audio");
        smooth_vf_swap_timing_set(false);
    }
}

/// FlowFPS outputs ~60 fps from the vf chain; **audio**-locked sync often collapses visible cadence.
/// Match presentation to the display timeline (`display-resample`) and disable shader **`interpolation`**.
/// **`hwdec`** / **`vd-lavc-dr`** stay whatever mpv already uses (typically auto).
fn apply_smooth_vf_present_opts(mpv: &Mpv) {
    if let Err(e) = mpv.set_property("video-sync", "display-resample") {
        if video_log() {
            eprintln!("[rhino] video: (verbose) video-sync display-resample failed: {e:?}");
        }
    }
    let _ = mpv.set_property("interpolation", "no");
    if video_log() {
        eprintln!(
            "[rhino] video: (verbose) smooth vf: video-sync=display-resample interpolation=no"
        );
    }
    // Enable swap reports last so **`report_swap`** cannot fire until **`display-resample`** is active.
    smooth_vf_swap_timing_set(true);
}

fn clear_vf(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool) {
    let inner = || {
        forget_bundled_me_budget_vf_apply();
        if let Err(e) = mpv.command("vf", &["clr", ""]) {
            eprintln!("[rhino] video: vf clr failed: {e:?}; trying set_property vf");
            if let Err(e2) = mpv.set_property("vf", "") {
                eprintln!("[rhino] video: set_property vf (clear) failed: {e2:?}");
            }
        } else if vlog {
            eprintln!("[rhino] video: vf clr ok");
        }
        let _ = mpv.set_property("vf", "");
        restore_non_smooth_present_opts(mpv);
    };
    #[cfg(target_os = "macos")]
    {
        if let Some(b) = bundle {
            b.with_macos_vf_teardown(inner);
            b.macos_ping_render_context();
            b.macos_mark_display_pending();
        } else {
            inner();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        inner();
    }
}

/// Clear **`vf vapoursynth`** before mpv **`loadfile`** replaces media so the new file is not decoded
/// through the previous clip's warm script (avoids wrong **`RHINO_SOURCE_FPS`** / duplicate preset lines).
pub fn strip_vapoursynth_before_replace_media(b: &MpvBundle) {
    if !vf_chain_has_vapoursynth(&b.mpv) {
        return;
    }
    clear_vf(&b.mpv, Some(b), video_log());
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

/// If Smooth 60 is on, **speed** is ~1.0×, and `vapoursynth` is still not in the `vf` list, run
/// [apply_mpv_video] once (covers a rare missed attach).
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
    if smooth_prefers_display_resample_bundle(mpv, Some(b)) {
        if vf_chain_has_vapoursynth(mpv) {
            apply_interleaved_display_resample(mpv, Some(b), video_log());
        }
        return MpvVideoApply::default();
    }
    if vf_chain_has_vapoursynth(mpv) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: reapply_60_if_still_missing → apply_mpv_video");
    apply_mpv_video(b, v, None)
}

/// Drop the vapoursynth `vf` immediately before a **seek** (or similar position jump) when it is
/// still present so mpv can decode a real frame — especially while **paused**. Plain pause/unpause
/// does not call this.
pub fn unload_smooth_on_pause(mpv: &Mpv) -> bool {
    mark_smooth_cadence_unstable_after_seek();
    if !vf_chain_has_vapoursynth(mpv) {
        return false;
    }
    let vlog = video_log();
    clear_vf(mpv, None, vlog);
    set_auto_decode(mpv, vlog);
    true
}
pub fn apply_mpv_video_init(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    apply_mpv_video_impl(mpv, None, v, None)
}

/// Normal playback is intentionally a no-op: leave mpv's timing, decode, and filter defaults alone.
/// When Smooth 60 is active, replace the `vf` list and add VapourSynth at ~**1.0×** only.
/// [speed_hint] is passed to [add_smooth_60] when set (e.g. header row) to match env before the [vf] add.
fn log_apply(v: &VideoPrefs) {
    if !video_log() {
        return;
    }
    eprintln!(
        "[rhino] video: apply_mpv_video smooth_60={} vs_path_len={}",
        v.smooth_60,
        v.vs_path.len()
    );
    if !v.smooth_60 {
        eprintln!(
            "[rhino] video: smooth_60 off — no 60 fps vf. Enable **Preferences** → **Smooth Video (60 FPS)** for VapourSynth (bundled .vpy if path is empty)."
        );
    }
}

pub fn apply_mpv_video(b: &MpvBundle, v: &mut VideoPrefs, speed_hint: Option<f64>) -> MpvVideoApply {
    apply_mpv_video_impl(&b.mpv, Some(b), v, speed_hint)
}

/// Interleaved / unstable cadence: strip VapourSynth; mpv **display-resample** only.
fn apply_interleaved_display_resample(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool) {
    if vf_chain_has_vapoursynth(mpv) {
        clear_vf(mpv, bundle, vlog);
    }
    apply_source_fps_env(None);
    apply_smooth_vf_present_opts(mpv);
    eprintln!(
        "[rhino] video: interleaved / unstable cadence — Smooth uses mpv display-resample (no VapourSynth)"
    );
}

fn apply_mpv_video_without_mvtools(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    paused: bool,
    want_60: bool,
    had_vapoursynth: bool,
    vlog: bool,
) -> MpvVideoApply {
    let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
    let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
    let keep_vf_during_pause = paused && want_60 && !display_only;
    let stripped_vf = had_vapoursynth && !keep_vf_during_pause;
    if stripped_vf {
        clear_vf(mpv, bundle, vlog);
        set_auto_decode(mpv, vlog);
        if !want_60 {
            smooth_off_refresh_playhead(mpv, bundle);
        }
    }
    if want_60 && eligible_1x && display_only {
        apply_interleaved_display_resample(mpv, bundle, vlog);
    } else if !want_60 && !stripped_vf {
        restore_non_smooth_present_opts(mpv);
    }
    post_smooth_60_state(mpv, v, want_60, false, vlog);
    MpvVideoApply::default()
}

fn apply_mpv_video_impl(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    let vlog = video_log();
    log_apply(v);
    let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
    let want_60 = v.smooth_60;
    let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
    let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
    let display_resample = want_60 && eligible_1x && display_only && !paused;
    let use_mvtools = want_60 && smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint) && !paused;
    let had_vapoursynth = vf_chain_has_vapoursynth(mpv);
    if display_resample {
        apply_interleaved_display_resample(mpv, bundle, vlog);
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply::default();
    }
    if !use_mvtools {
        return apply_mpv_video_without_mvtools(
            mpv,
            bundle,
            v,
            speed_hint,
            paused,
            want_60,
            had_vapoursynth,
            vlog,
        );
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    if had_vapoursynth && vf_smooth_matches_prefs(mpv, v, bundle) {
        if smooth_prefers_display_resample_bundle(mpv, bundle) {
            apply_interleaved_display_resample(mpv, bundle, vlog);
            post_smooth_60_state(mpv, v, want_60, false, vlog);
            return MpvVideoApply::default();
        }
        match speed_hint {
            Some(s) => set_playback_speed_env(s),
            None => set_playback_speed_env_from_mpv(mpv),
        }
        let smooth_cap = effective_smooth_me_budget_px(mpv, v, bundle);
        let fps_opt = source_fps_from_mpv(mpv);
        if v.vs_path.trim().is_empty() {
            crate::paths::publish_smooth_me_budget_env(smooth_cap);
        }
        let fps_env_before = std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR).ok();
        let before_hz = fps_env_before
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|x| x.is_finite());
        let cadence_unchanged = match (fps_opt, before_hz) {
            (Some(w), Some(b)) => (w - b).abs() < 1e-5,
            (None, None) => true,
            _ => false,
        };
        apply_source_fps_env(fps_opt);
        // `RHINO_SOURCE_FPS` is read when the `.vpy` graph starts; refreshing env alone does not
        // re-run the script after `vf add`. Rebuild when the **numeric** cadence changed — not when
        // the var was empty and we only re-published the same Hz (warm reopen / redundant resync).
        if cadence_unchanged {
            apply_smooth_vf_present_opts(mpv);
            post_smooth_60_state(mpv, v, want_60, false, vlog);
            return MpvVideoApply::default();
        }
        eprintln!(
            "[rhino] video: rebuilding vapoursynth vf ({} changed)",
            crate::paths::RHINO_SOURCE_FPS_VAR
        );
        clear_vf(mpv, bundle, vlog);
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    // Smooth vf presentation + swap timing; stripping vf restores plain opts (clear_vf).
    clear_vf(mpv, bundle, vlog);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}
