pub fn apply_mpv_video_init(mpv: &Mpv, v: &mut VideoPrefs) -> MpvVideoApply {
    apply_mpv_video_impl(mpv, None, None, v, None)
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

pub fn apply_mpv_video(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return MpvVideoApply::default();
    };
    apply_mpv_video_impl(&b.mpv, Some(b), Some(player), v, speed_hint)
}

/// First Smooth-on while playing (graph never stripped this open): **`loadfile replace`** at the
/// playhead so **`vf add`** runs after resume (A/V aligned). When
/// [MpvBundle::smooth_vf_stripped_this_open] is set (Smooth off→on, post-seek), returns **false** so
/// the caller runs [apply_mpv_video] → [smooth_reattach_after_vf_strip] (**`vf add`** only).
///
/// Returns **true** when reload started — skip **`apply_mpv_video`**; FileLoaded resync attaches **`vf`**.
pub fn smooth_user_enable_playing_reset(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    v: &mut VideoPrefs,
) -> bool {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    if !v.smooth_60
        || mpv.get_property::<bool>("pause").unwrap_or(true)
        || vf_chain_has_vapoursynth(mpv)
        || !mpv_has_open_media(mpv)
        || b.smooth_vf_stripped_this_open()
    {
        return false;
    }
    if prep_smooth_60_for_vf(mpv, v, None, Some(b), None) {
        return false;
    }
    if reload_open_media_for_vf_reset(b, true) {
        eprintln!("[rhino] video: smooth-on loadfile reset (user toggle while playing)");
        return true;
    }
    eprintln!("[rhino] video: smooth-on loadfile reset failed — apply will try live vf add");
    false
}

fn apply_mpv_video_impl(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    player: Option<&Rc<RefCell<Option<MpvBundle>>>>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
) -> MpvVideoApply {
    let vlog = video_log();
    log_apply(v);
    if bundle.is_some_and(|b| b.smooth_vf_attach_pending()) {
        eprintln!("[rhino] video: apply_mpv_video skipped (vapoursynth attach in flight)");
        return MpvVideoApply::default();
    }
    let paused = mpv.get_property::<bool>("pause").unwrap_or(true);
    let want_60 = v.smooth_60;
    let cadence_hz = want_60.then(|| refresh_smooth_cadence_gate(mpv, bundle)).flatten();
    let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
    let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
    let display_resample = want_60 && eligible_1x && display_only && !paused;
    let had_vapoursynth = vf_chain_has_vapoursynth(mpv);
    let use_mvtools = want_60
        && smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint)
        && (!paused || !had_vapoursynth);
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

    let Some(pl) = player else {
        eprintln!("[rhino] video: smooth vf rebuild skipped (no player handle for A/V resync)");
        return MpvVideoApply::default();
    };

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
        if cadence_unchanged {
            apply_smooth_vf_present_opts(mpv);
            post_smooth_60_state(mpv, v, want_60, false, vlog);
            return MpvVideoApply::default();
        }
        eprintln!(
            "[rhino] video: rebuilding vapoursynth vf ({} changed)",
            crate::paths::RHINO_SOURCE_FPS_VAR
        );
        let disabled_60 = rebuild_smooth_vf_chain(pl, mpv, bundle, v, speed_hint, cadence_hz, vlog);
        post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
        return MpvVideoApply {
            smooth_auto_off: disabled_60,
        };
    }

    let disabled_60 = rebuild_smooth_vf_chain(pl, mpv, bundle, v, speed_hint, cadence_hz, vlog);
    post_smooth_60_state(mpv, v, want_60, disabled_60, vlog);
    MpvVideoApply {
        smooth_auto_off: disabled_60,
    }
}
