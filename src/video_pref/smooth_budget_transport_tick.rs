/// **`1 Hz`** transport tick (**bundled** `.vpy` only; **caller** skips ticks while the playback shell window is inactive / unmapped—see **`transport_events`**): tighten ME budget when the **playback smoothness strain tally**
/// shows **>** **`OVERLOAD_STRAIN_GT_FRAC`** strict rolling strain for **`OVERLOAD_FIRE_STREAK_TICKS`** successive ticks; relax when relaxed-window strain **\<** **`RECOVERY_STRAIN_LT_FRAC`** **three hundred** successive ticks.
/// Busy-system pauses are maintained first (see **`smooth_load_hold`**).
pub(crate) fn smooth_budget_on_transport_tick(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    pause: bool,
    core_idle: bool,
    state_cell: &RefCell<SmoothBudgetDecoderState>,
) {
    let process_cpu_frac = {
        let mut st = state_cell.borrow_mut();
        smooth_budget_refresh_process_cpu_frac(&mut st)
    };
    if smooth_load_hold_on_tick(player, video_pref) {
        return;
    }

    if transport_tick_inactive(video_pref, pause, core_idle) {
        return;
    }
    let Some(sample) = player.try_borrow().ok().and_then(|g| {
        g.as_ref()
            .and_then(|b| collect_transport_tick_sample(b, video_pref))
    }) else {
        return;
    };
    let now = Instant::now();
    let fires = collect_tick_fires(state_cell, &sample, now);
    let o = transport_budget_outcome(sample, now, process_cpu_frac, fires);
    apply_budget_actions_after_sample(player, video_pref, state_cell, &o);
}

/// Skip ticks while the core cannot decode (paused / idle) or the pref is not bundled Smooth.
fn transport_tick_inactive(
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    pause: bool,
    core_idle: bool,
) -> bool {
    if pause || core_idle {
        return true;
    }
    let vp = video_pref.borrow();
    !vp.smooth_60 || !vp.vs_path.trim().is_empty()
}

/// mpv-side inputs for one tick, collected while the player borrow is still held.
struct TransportTickSample {
    snap: SmoothBudgetSignalSnap,
    decode_fps: f64,
    decode_px: Option<u64>,
    current_budget_px: u64,
}

/// Only the bundled graph on eligible media samples the budget: vapoursynth present and wanted
/// ([smooth_wants_vapoursynth_vf]), with a readable strain signal.
fn collect_transport_tick_sample(
    b: &crate::mpv_embed::MpvBundle,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
) -> Option<TransportTickSample> {
    if !vf_chain_has_vapoursynth(&b.mpv) || !smooth_wants_vapoursynth_vf(&b.mpv, Some(b), None) {
        return None;
    }
    let snap = read_smooth_budget_signal(&b.mpv)?;
    let decode_fps = playback_fps_for_decode_budget(&b.mpv);
    let decode_px = decode_pixel_area_for_me_budget(&b.mpv);
    let current_budget_px = {
        let vp = video_pref.borrow();
        effective_smooth_me_budget_px(&b.mpv, &vp, Some(b))
    };
    Some(TransportTickSample {
        snap,
        decode_fps,
        decode_px,
        current_budget_px,
    })
}

/// Fire flags and strain gates for one tick: window sample + throttled stats line, then the
/// recovery gate re-read (overload handling may have just set it).
struct TickFires {
    rate_opt: Option<f64>,
    overload_fire: bool,
    recover_fire: bool,
    allow_recovery_raise: bool,
    recovery_blocked_after_overload_this_open: bool,
}

fn collect_tick_fires(
    state_cell: &RefCell<SmoothBudgetDecoderState>,
    sample: &TransportTickSample,
    now: Instant,
) -> TickFires {
    let allow_recovery_raise =
        raised_me_budget_can_reduce_downscale(sample.decode_px, sample.current_budget_px);
    let (rate_opt, overload_fire, recover_fire) =
        sample_and_emit_drop_stats(state_cell, &sample.snap, now, sample.decode_fps);
    TickFires {
        rate_opt,
        overload_fire,
        recover_fire,
        allow_recovery_raise,
        recovery_blocked_after_overload_this_open: state_cell
            .borrow()
            .recovery_blocked_after_overload_this_open,
    }
}

/// Advance the rolling window for this tick and emit throttled drop stats.
/// Returns `(strict_rate, overload_fire, recover_fire)` from [sample_window_and_fire_flags].
fn sample_and_emit_drop_stats(
    state_cell: &RefCell<SmoothBudgetDecoderState>,
    snap: &SmoothBudgetSignalSnap,
    now: Instant,
    fps: f64,
) -> (Option<f64>, bool, bool) {
    let recovery_blocked_snapshot = state_cell
        .borrow()
        .recovery_blocked_after_overload_this_open;
    let mut st = state_cell.borrow_mut();
    let out = sample_window_and_fire_flags(&mut st, snap.primary, now, fps, snap.src);
    maybe_emit_smooth_drop_stats_line(&mut st, snap, fps, now, recovery_blocked_snapshot, out.0);
    out
}

/// Bundle the sampled inputs with the fire flags into the decision outcome.
fn transport_budget_outcome(
    sample: TransportTickSample,
    now: Instant,
    process_cpu_frac: Option<f64>,
    fires: TickFires,
) -> TransportBudgetOutcome {
    TransportBudgetOutcome {
        current_budget_px: sample.current_budget_px,
        cur_count: sample.snap.primary,
        now,
        rate_opt: fires.rate_opt,
        overload_fire: fires.overload_fire,
        recover_fire: fires.recover_fire,
        allow_recovery_raise: fires.allow_recovery_raise,
        recovery_blocked_after_overload_this_open: fires.recovery_blocked_after_overload_this_open,
        process_cpu_frac,
        snap: sample.snap,
        decode_fps: sample.decode_fps,
        decode_px: sample.decode_px,
    }
}
