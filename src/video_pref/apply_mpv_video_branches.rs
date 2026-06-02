// Branch handlers for [apply_mpv_video_impl]: the matched-graph refresh, the Smooth-off teardown,
// and the non-MVTools (display-resample / plain) path. The dispatcher and the `vf` rebuild stay in
// `decode_and_apply_mpv_video.rs`; these are the leaf outcomes it routes to.

/// Smooth on, graph already matches prefs: refresh present opts / ME budget, and rebuild the `vf`
/// only when the source cadence (`RHINO_SOURCE_FPS`) actually changed.
fn apply_mpv_video_matched_vf(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
    want_60: bool,
    vlog: bool,
) -> MpvVideoApply {
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
    if cadence_unchanged {
        set_present_opts(mpv, true);
        post_smooth_60_state(mpv, v, want_60, false, vlog);
        return MpvVideoApply::default();
    }
    eprintln!(
        "[rhino] video: rebuilding vapoursynth vf ({} changed)",
        crate::paths::RHINO_SOURCE_FPS_VAR
    );
    let disabled_60 = rebuild_mvtools_vf_chain(mpv, bundle, v, speed_hint, cadence_hz, vlog);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}

/// Smooth off: clear the vapoursynth `vf` (pausing across the seek so audio cannot run ahead) and
/// restore plain present opts. No-op when no graph is present and none is pending.
fn apply_mpv_video_smooth_off(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    had_vapoursynth: bool,
    vlog: bool,
) -> MpvVideoApply {
    let attach_pending = bundle.is_some_and(|b| b.smooth_vf_attach_pending());
    if !had_vapoursynth && !attach_pending {
        post_smooth_60_state(mpv, v, false, false, vlog);
        return MpvVideoApply::default();
    }
    eprintln!("[rhino] video: smooth off — clearing vapoursynth vf");
    let snap = had_vapoursynth.then(|| vf_av_pause_begin(mpv));
    clear_vf(mpv, bundle, vlog);
    set_auto_decode(mpv, vlog);
    if let Some(s) = snap {
        vf_av_resume_end(mpv, bundle, &s, "smooth-off");
    }
    if !bluray_playback_active(mpv, bundle) {
        set_present_opts(mpv, false);
    }
    post_smooth_60_state(mpv, v, false, false, vlog);
    MpvVideoApply::default()
}

/// Smooth wants no MVTools graph here (paused-with-graph kept, unstable cadence → display-resample,
/// or plain playback): strip a stale graph when needed and apply the matching present opts.
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
            set_present_opts(mpv, false);
        }
    }
    post_smooth_60_state(mpv, v, want_60, false, vlog);
    MpvVideoApply::default()
}
