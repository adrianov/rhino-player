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
    } else if !want_60 {
        sync_bluray_deinterlace_mpv(mpv, bundle);
        if !bluray_playback_active(mpv, bundle) && !stripped_vf {
            restore_non_smooth_present_opts(mpv);
        }
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
    if mpv_has_open_media(mpv) {
        sync_bluray_deinterlace_mpv(mpv, bundle);
    }
    let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
    let want_60 = v.smooth_60;
    let cadence_hz = want_60.then(|| refresh_smooth_cadence_gate(mpv, bundle)).flatten();
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
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
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
        let fps_opt = cadence_hz;
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
        sync_bluray_deinterlace_mpv(mpv, bundle);
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    // Smooth vf presentation + swap timing; stripping vf restores plain opts (clear_vf).
    clear_vf(mpv, bundle, vlog);
    sync_bluray_deinterlace_mpv(mpv, bundle);
    let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
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
    if vf_smooth_matches_prefs(mpv, v, Some(b)) && !smooth_prefers_display_resample_bundle(mpv, Some(b)) {
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: reapply_60_if_still_missing → apply_mpv_video");
    apply_mpv_video(b, v, None)
}
