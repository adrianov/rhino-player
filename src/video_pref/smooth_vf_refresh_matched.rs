// In-place refresh of an already-matching Smooth vf chain (no full rebuild).

/// The existing vapoursynth chain already matches prefs: refresh env/opts in place.
/// Returns [None] when the caller must run a full [rebuild_smooth_vf_chain].
fn refresh_matched_smooth_vf(
    pl: &Rc<RefCell<Option<MpvBundle>>>,
    p: &mut SmoothVfParams<'_>,
) -> Option<MpvVideoApply> {
    if smooth_prefers_display_resample_bundle(p.mpv, p.bundle) {
        apply_interleaved_display_resample(p.mpv, p.bundle, p.vlog);
        post_smooth_60_state(p.mpv, p.v, p.want_60, false, p.vlog);
        return Some(MpvVideoApply::default());
    }
    let cadence_unchanged = publish_smooth_envs(p.mpv, p.v, p.bundle, p.speed_hint, p.cadence_hz);
    if cadence_unchanged {
        apply_smooth_vf_present_opts(p.mpv);
        post_smooth_60_state(p.mpv, p.v, p.want_60, false, p.vlog);
        return Some(MpvVideoApply::default());
    }
    eprintln!(
        "[rhino] video: rebuilding vapoursynth vf ({} changed)",
        crate::paths::RHINO_SOURCE_FPS_VAR
    );
    let disabled_60 =
        rebuild_smooth_vf_chain(pl, p.mpv, p.bundle, p.v, p.speed_hint, p.cadence_hz, p.vlog);
    post_smooth_60_state(p.mpv, p.v, p.want_60, disabled_60, p.vlog);
    Some(MpvVideoApply {
        smooth_auto_off: disabled_60,
    })
}

/// True when the requested cadence equals the currently published source-fps env.
fn cadence_matches_source_fps_env(fps_opt: Option<f64>) -> bool {
    let before_hz = std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|x| x.is_finite());
    match (fps_opt, before_hz) {
        (Some(w), Some(b)) => (w - b).abs() < 1e-5,
        (None, None) => true,
        _ => false,
    }
}

/// Returns whether the requested cadence equals the previously published source fps.
fn publish_smooth_envs(
    mpv: &Mpv,
    v: &VideoPrefs,
    bundle: Option<&MpvBundle>,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
) -> bool {
    match speed_hint {
        Some(s) => set_playback_speed_env(s),
        None => set_playback_speed_env_from_mpv(mpv),
    }
    let smooth_cap = effective_smooth_me_budget_px(mpv, v, bundle);
    if v.vs_path.trim().is_empty() {
        crate::paths::publish_smooth_me_budget_env(smooth_cap);
    }
    let cadence_unchanged = cadence_matches_source_fps_env(cadence_hz);
    apply_source_fps_env(cadence_hz);
    cadence_unchanged
}
