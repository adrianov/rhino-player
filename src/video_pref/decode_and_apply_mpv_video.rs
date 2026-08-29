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
/// the caller runs [apply_mpv_video] → [smooth_reattach_after_vf_strip] (**`vf add`**; toggle on may
/// exact-seek the playhead for A/V).
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

include!("smooth_apply_plan.rs");
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
    let plan = SmoothApplyPlan::probe(mpv, bundle, v, speed_hint);
    if let Some(outcome) = apply_plan_fast_path(mpv, bundle, v, speed_hint, &plan, vlog) {
        return outcome;
    }
    if !mpv_has_open_media(mpv) {
        let disabled_60 = add_smooth_60(mpv, v, speed_hint, bundle, plan.cadence_hz);
        return finish_smooth_apply(disabled_60, mpv, v, plan.want_60, vlog);
    }

    let mut p = SmoothVfParams {
        mpv,
        bundle,
        v,
        speed_hint,
        cadence_hz: plan.cadence_hz,
        want_60: plan.want_60,
        had_vapoursynth: plan.had_vapoursynth,
        vlog,
    };
    apply_smooth_vf_with_media(player, &mut p)
}

/// Branches that finish without touching a Smooth vf chain: display resampling and
/// builds with MVTools unavailable.
fn apply_plan_fast_path(
    mpv: &Mpv,
    bundle: Option<&MpvBundle>,
    v: &mut VideoPrefs,
    speed_hint: Option<f64>,
    plan: &SmoothApplyPlan,
    vlog: bool,
) -> Option<MpvVideoApply> {
    if plan.display_resample {
        apply_interleaved_display_resample(mpv, bundle, vlog);
        return Some(finish_smooth_apply(false, mpv, v, plan.want_60, vlog));
    }
    if !plan.use_mvtools {
        return Some(apply_mpv_video_without_mvtools(
            mpv,
            bundle,
            v,
            speed_hint,
            plan.want_60,
            plan.had_vapoursynth,
            vlog,
        ));
    }
    None
}

/// Parameters shared by the Smooth vf with-media refresh/rebuild paths.
struct SmoothVfParams<'a> {
    mpv: &'a Mpv,
    bundle: Option<&'a MpvBundle>,
    v: &'a mut VideoPrefs,
    speed_hint: Option<f64>,
    cadence_hz: Option<f64>,
    want_60: bool,
    had_vapoursynth: bool,
    vlog: bool,
}

/// Smooth MVTools path with media open: refresh a matching chain in place, else full rebuild.
fn apply_smooth_vf_with_media(
    player: Option<&Rc<RefCell<Option<MpvBundle>>>>,
    p: &mut SmoothVfParams<'_>,
) -> MpvVideoApply {
    let Some(pl) = player else {
        eprintln!("[rhino] video: smooth vf rebuild skipped (no player handle for A/V resync)");
        return MpvVideoApply::default();
    };

    if p.had_vapoursynth && vf_smooth_matches_prefs(p.mpv, p.v, p.bundle) {
        if let Some(applied) = refresh_matched_smooth_vf(pl, p) {
            return applied;
        }
    }

    let disabled_60 =
        rebuild_smooth_vf_chain(pl, p.mpv, p.bundle, p.v, p.speed_hint, p.cadence_hz, p.vlog);
    finish_smooth_apply(disabled_60, p.mpv, p.v, p.want_60, p.vlog)
}

include!("smooth_vf_refresh_matched.rs");
