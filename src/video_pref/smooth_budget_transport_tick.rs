/// **`1 Hz`** transport tick (**bundled** `.vpy` only; **caller** skips ticks while the playback shell window is inactive / unmapped—see **`transport_events`**): tighten ME budget when the **playback smoothness strain tally**
/// shows **>** **`OVERLOAD_STRAIN_GT_FRAC`** strict rolling strain **five** successive ticks; relax when relaxed-window strain **\<** **`RECOVERY_STRAIN_LT_FRAC`** **three hundred** successive ticks.
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
    if smooth_load_hold_on_tick(player, video_pref, process_cpu_frac) {
        return;
    }

    if pause || core_idle {
        return;
    }

    {
        let vp = video_pref.borrow();
        if !vp.smooth_60 || !vp.vs_path.trim().is_empty() {
            return;
        }
    }

    let Ok(g) = player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    if !vf_chain_has_vapoursynth(&b.mpv)
        || !smooth_wants_vapoursynth_vf(&b.mpv, Some(b), None)
    {
        return;
    }
    let Some(snap) = read_smooth_budget_signal(&b.mpv) else {
        return;
    };
    let cur_count = snap.primary;
    let fps = playback_fps_for_decode_budget(&b.mpv);
    let decode_px = decode_pixel_area_for_me_budget(&b.mpv);
    let current_budget_px = {
        let vp = video_pref.borrow();
        effective_smooth_me_budget_px(&b.mpv, &vp, Some(b))
    };
    drop(g);
    let now = Instant::now();
    let allow_recovery_raise = raised_me_budget_can_reduce_downscale(decode_px, current_budget_px);
    let recovery_blocked_after_overload_snapshot =
        state_cell.borrow().recovery_blocked_after_overload_this_open;
    let (rate_opt, overload_fire, recover_fire) = {
        let mut st = state_cell.borrow_mut();
        let out = sample_window_and_fire_flags(&mut st, cur_count, now, fps, snap.src);
        maybe_emit_smooth_drop_stats_line(
            &mut st,
            &snap,
            fps,
            now,
            recovery_blocked_after_overload_snapshot,
            out.0,
        );
        out
    };

    let recovery_blocked_after_overload_this_open =
        state_cell.borrow().recovery_blocked_after_overload_this_open;

    let o = TransportBudgetOutcome {
        current_budget_px,
        cur_count,
        now,
        rate_opt,
        overload_fire,
        recover_fire,
        allow_recovery_raise,
        recovery_blocked_after_overload_this_open,
        process_cpu_frac,
        snap,
        decode_fps: fps,
        decode_px,
    };

    apply_budget_actions_after_sample(player, video_pref, state_cell, &o);
}
