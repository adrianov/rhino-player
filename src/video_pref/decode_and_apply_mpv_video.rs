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

/// User enabled Smooth while playing: reload at playhead so **`vf add`** runs after resume (A/V aligned).
/// Returns **true** when **`loadfile replace`** started — caller must skip **`apply_mpv_video`** (FileLoaded resync attaches **`vf`**).
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

/// Seek (or Smooth off→on) stripped the graph: defer + keyframe-seek reattach — never a
/// **`loadfile`** here. The reload fallback lives in [add_smooth_60]'s **`vf add`**-failure branch.
fn smooth_reattach_after_vf_strip(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
) -> bool {
    let snap = vf_swap_snap(mpv, true);
    let disabled_60 = prep_smooth_60_for_vf(mpv, v, speed_hint, bundle, cadence_hz);
    if disabled_60 {
        vf_swap_unpause(mpv, &snap);
        return true;
    }
    eprintln!("[rhino] video: deferred smooth reattach after vf strip");
    defer_smooth_vf_swap(player, mpv, bundle, snap, true, "smooth-reattach");
    false
}

fn add_smooth_60_with_av_log(
    mpv: &Mpv,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    bundle: Option<&MpvBundle>,
    cadence_hz: Option<f64>,
) -> bool {
    let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
    if !disabled_60 {
        log_smooth_avsync(mpv);
        vf_av_ping_render(bundle);
    }
    disabled_60
}

/// First attach (open / smooth-on after off): **`vf add`** immediately. Replacing a live graph: defer + keyframe.
fn rebuild_smooth_vf_chain(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
    vlog: bool,
) -> bool {
    if vf_swap_post_seek_attach_active() {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
        vf_swap_clear_post_seek_attach();
        if !disabled_60 {
            log_smooth_avsync(mpv);
            vf_av_ping_render(bundle);
        }
        return disabled_60;
    }
    if vf_swap_defer_in_flight() {
        return false;
    }
    let had_vf = vf_chain_has_vapoursynth(mpv);
    if !had_vf {
        if bundle.is_some_and(|b| b.smooth_vf_stripped_this_open()) {
            return smooth_reattach_after_vf_strip(player, mpv, bundle, v, speed_hint, cadence_hz);
        }
        return add_smooth_60_with_av_log(mpv, v, speed_hint, bundle, cadence_hz);
    }
    let snap = vf_swap_snap(mpv, true);
    let disabled_60 = prep_smooth_60_for_vf(mpv, v, speed_hint, bundle, cadence_hz);
    if disabled_60 {
        vf_swap_unpause(mpv, &snap);
        return true;
    }
    clear_vf(mpv, bundle, vlog);
    defer_smooth_vf_swap(player, mpv, bundle, snap, true, "smooth-swap");
    false
}

fn apply_mpv_video_without_mvtools(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    _paused: bool,
    want_60: bool,
    had_vapoursynth: bool,
    vlog: bool,
) -> MpvVideoApply {
    let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
    let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
    let keep_vf = want_60 && eligible_1x && !display_only;
    let stripped_vf = had_vapoursynth && !keep_vf;
    if stripped_vf {
        if let Some(b) = bundle {
            b.set_smooth_vf_stripped_this_open(true);
            b.clear_smooth_vf_reload_attempted();
        }
        let snap = vf_swap_snap(mpv, true);
        clear_vf(mpv, bundle, vlog);
        set_auto_decode(mpv, vlog);
        if !want_60 {
            smooth_off_refresh_playhead(mpv, bundle, &snap);
        } else {
            vf_swap_unpause(mpv, &snap);
            vf_av_ping_render(bundle);
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
