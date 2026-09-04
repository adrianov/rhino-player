/// Graph was stripped (seek / Smooth off→on): **`vf add`** — never **`loadfile`**. Seek reattach
/// leaves pause as-is. User toggle **on** ([take_reattach_av_resync]): pause, attach, exact seek
/// to the pre-attach playhead, unpause — bare **`vf add`** while playing can leave A/V drifted.
/// Reload on **`vf add`** failure lives in [add_smooth_60].
fn smooth_reattach_after_vf_strip(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
) -> bool {
    let av_resync = take_reattach_av_resync();
    let snap = av_resync.then(|| vf_swap_snap(mpv, true));
    let playhead = av_resync
        .then(|| vf_resync_playhead_sec(mpv, bundle))
        .flatten();
    eprintln!(
        "[rhino] video: smooth reattach after vf strip{}",
        if av_resync { " (toggle A/V resync)" } else { "" }
    );
    finish_reattach_after_add(
        mpv,
        bundle,
        snap.as_ref(),
        playhead,
        add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz),
    )
}

/// Exact playhead seek + unpause after a toggle-armed reattach; seek reattach only logs A/V.
fn finish_reattach_after_add(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    snap: Option<&VfAvSnap>,
    playhead: Option<f64>,
    disabled_60: bool,
) -> bool {
    if disabled_60 {
        if let Some(s) = snap {
            vf_swap_unpause(mpv, s);
        }
        return true;
    }
    if let (Some(s), Some(t)) = (snap, playhead) {
        exact_playhead_resync(mpv, bundle, s, t, "smooth reattach");
        return false;
    }
    if let Some(s) = snap {
        eprintln!("[rhino] video: smooth reattach playhead resync skipped (no playhead)");
        vf_swap_unpause(mpv, s);
    }
    log_smooth_avsync(mpv);
    vf_av_ping_render(bundle);
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

/// First attach (open / first Smooth-on): **`vf add`** immediately. After a strip: [smooth_reattach_after_vf_strip].
/// Replacing a live graph: defer + keyframe.
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
        return rebuild_post_seek_attach(mpv, bundle, v, speed_hint, cadence_hz);
    }
    if vf_swap_defer_in_flight() {
        return false;
    }
    if !vf_chain_has_vapoursynth(mpv) {
        if bundle.is_some_and(|b| b.smooth_vf_stripped_this_open()) {
            return smooth_reattach_after_vf_strip(mpv, bundle, v, speed_hint, cadence_hz);
        }
        return add_smooth_60_with_av_log(mpv, v, speed_hint, bundle, cadence_hz);
    }
    let snap = vf_swap_snap(mpv, true);
    if prep_smooth_60_for_vf(mpv, v, speed_hint, bundle, cadence_hz) {
        vf_swap_unpause(mpv, &snap);
        return true;
    }
    clear_vf(mpv, bundle, vlog);
    defer_smooth_vf_swap(player, mpv, bundle, snap, true, "smooth-swap");
    false
}

/// Deferred attach armed for after a seek: add now and clear the pending flag.
fn rebuild_post_seek_attach(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
) -> bool {
    let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, cadence_hz);
    vf_swap_clear_post_seek_attach();
    if !disabled_60 {
        log_smooth_avsync(mpv);
        vf_av_ping_render(bundle);
    }
    disabled_60
}

/// Live graph no longer wanted: strip it (snap/pause, clear, auto decode) and refresh or unpause.
fn strip_unwanted_smooth_vf(mpv: &Mpv, bundle: Option<&MpvBundle>, vlog: bool, want_60: bool) {
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

fn apply_mpv_video_without_mvtools(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    want_60: bool,
    had_vapoursynth: bool,
    vlog: bool,
) -> MpvVideoApply {
    let eligible_1x = mvtools_vf_eligible(mpv, speed_hint);
    let display_only = smooth_prefers_display_resample_bundle(mpv, bundle);
    // Respect load-hold / cadence gates — same eligibility as attach (`smooth_wants_vapoursynth_vf`).
    let keep_vf = want_60 && smooth_wants_vapoursynth_vf(mpv, bundle, speed_hint);
    let stripped_vf = had_vapoursynth && !keep_vf;
    if stripped_vf {
        strip_unwanted_smooth_vf(mpv, bundle, vlog, want_60);
    }
    apply_non_smooth_present_mode(
        mpv,
        bundle,
        vlog,
        want_60,
        eligible_1x,
        display_only,
        stripped_vf,
    );
    post_smooth_60_state(mpv, v, want_60, false, vlog);
    MpvVideoApply::default()
}

/// Interleaved cadence → display-resample; Smooth off → present-opts restore
/// (unless just stripped or a disc is playing).
fn apply_non_smooth_present_mode(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    vlog: bool,
    want_60: bool,
    eligible_1x: bool,
    display_only: bool,
    stripped_vf: bool,
) {
    if want_60 && eligible_1x && display_only {
        apply_interleaved_display_resample(mpv, bundle, vlog);
    } else if !want_60 && !bluray_playback_active(mpv, bundle) && !stripped_vf {
        restore_non_smooth_present_opts(mpv);
    }
}
