/// Graph was stripped (seek / Smooth off→on): **`vf add`** only — never **`loadfile`**, a second
/// keyframe seek, or **`vf_swap_snap` pause**. Leave pause as-is so a playing seek keeps playing.
/// Reload on **`vf add`** failure lives in [add_smooth_60].
fn smooth_reattach_after_vf_strip(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
) -> bool {
    eprintln!("[rhino] video: smooth reattach after vf strip");
    add_smooth_60_with_av_log(mpv, v, speed_hint, bundle, cadence_hz)
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
            return smooth_reattach_after_vf_strip(mpv, bundle, v, speed_hint, cadence_hz);
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
