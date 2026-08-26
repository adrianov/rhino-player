// Apply-time planning: one mpv probe pass feeding the Smooth apply decision,
// plus the shared post-apply result finalizer.

/// Facts and derived flags probed once per [apply_mpv_video_impl], in mpv-query order.
struct SmoothApplyPlan {
    want_60: bool,
    cadence_hz: Option<f64>,
    display_resample: bool,
    use_mvtools: bool,
    had_vapoursynth: bool,
}

impl SmoothApplyPlan {
    fn probe(
        mpv: &Mpv,
        bundle: Option<&MpvBundle>,
        v: &VideoPrefs,
        speed_hint: Option<f64>,
    ) -> Self {
        let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
        let want_60 = v.smooth_60;
        let cadence_hz = want_60
            .then(|| refresh_smooth_cadence_gate(mpv, bundle))
            .flatten();
        let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
        let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
        let had_vapoursynth = vf_chain_has_vapoursynth(mpv);
        let display_resample = want_60 && eligible_1x && display_only && !paused;
        let use_mvtools = want_60
            && smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint)
            && (!paused || !had_vapoursynth);
        Self {
            want_60,
            cadence_hz,
            display_resample,
            use_mvtools,
            had_vapoursynth,
        }
    }
}


/// Shared tail: report post-apply state and surface an auto-disable to the UI.
fn finish_smooth_apply(
    disabled_60: bool,
    mpv: &Mpv,
    v: &VideoPrefs,
    want_60: bool,
    vlog: bool,
) -> MpvVideoApply {
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}
