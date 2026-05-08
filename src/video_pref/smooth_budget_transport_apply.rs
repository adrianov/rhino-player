struct TransportBudgetOutcome {
    current_budget_px: u64,
    cur_count: u64,
    now: Instant,
    rate_opt: Option<f64>,
    recovery_rate_opt: Option<f64>,
    overload_fire: bool,
    recover_fire: bool,
    allow_recovery_raise: bool,
    recovery_blocked_after_overload_this_open: bool,
    process_cpu_frac: Option<f64>,
    snap: SmoothBudgetSignalSnap,
    decode_fps: f64,
    decode_px: Option<u64>,
    overload_streak: u32,
    recovery_quiet_streak: u32,
    overload_window_ready: bool,
    recovery_window_ready: bool,
}

fn apply_budget_actions_after_sample(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    state_cell: &RefCell<SmoothBudgetDecoderState>,
    o: &TransportBudgetOutcome,
) {
    if o.overload_fire {
        match o.rate_opt.filter(|r| *r > OVERLOAD_STRAIN_GT_FRAC) {
            Some(r_high) => {
                let cand = budget_after_decoder_overload(o.current_budget_px, r_high);
                eprintln_smooth_budget_overload(&o.snap, o.decode_fps, r_high, o.current_budget_px, cand);
                if cand < o.current_budget_px {
                    state_cell.borrow_mut().recovery_blocked_after_overload_this_open = true;
                }
                let _ = persist_budget_and_maybe_rebuild_vf(
                    player,
                    video_pref,
                    cand,
                    "(smooth playback timing strain — lowering ME budget)",
                );
                reset_decoder_state_after_shrink(state_cell, o.now, o.cur_count);
            }
            None => {
                eprintln!(
                    "[rhino] smooth: decision hold anomaly overload_fire_but_no_strain_gt_40pct rate_opt {:?}",
                    o.rate_opt,
                );
            }
        }
        return;
    }

    if o.recover_fire {
        state_cell.borrow_mut().recovery_quiet_streak = 0;
        maybe_handle_recovery_raise(player, video_pref, o);
        return;
    }

    eprintln_smooth_budget_hold_line(o);
}

fn reset_decoder_state_after_shrink(cell: &RefCell<SmoothBudgetDecoderState>, now: Instant, cur_count: u64) {
    let mut st = cell.borrow_mut();
    st.recovery_quiet_streak = 0;
    st.samples.clear();
    st.samples.push_back((now, cur_count));
    st.overload_streak = 0;
    reset_smooth_drop_stats_window(&mut st);
}

fn maybe_handle_recovery_raise(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    o: &TransportBudgetOutcome,
) {
    if !o.allow_recovery_raise {
        eprintln_smooth_budget_recovery_skip_decode_fits_cap(
            &o.snap,
            o.decode_fps,
            o.rate_opt,
            o.decode_px,
            o.current_budget_px.max(crate::db::MIN_SMOOTH_MAX_AREA),
        );
        return;
    }
    if o.recovery_blocked_after_overload_this_open {
        eprintln_smooth_budget_recovery_skip_after_overload_session(
            &o.snap,
            o.decode_fps,
            o.recovery_rate_opt,
            o.decode_px,
        );
        return;
    }
    if o.process_cpu_frac.is_some_and(|f| f > RECOVER_CPU_SHARE_HARD_MAX_FRAC) {
        eprintln_smooth_budget_recovery_skip_high_cpu(
            &o.snap,
            o.decode_fps,
            o.process_cpu_frac,
            RECOVER_CPU_SHARE_HARD_MAX_FRAC,
        );
        return;
    }
    maybe_raise_budget(video_pref, player, o);
}

fn maybe_raise_budget(
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    o: &TransportBudgetOutcome,
) {
    let Some(recover_to) = recovery_candidate(o.current_budget_px, o.decode_px) else {
        eprintln_smooth_budget_recovery_at_ceiling(&o.snap, o.current_budget_px, o.decode_px);
        return;
    };
    if recover_to <= o.current_budget_px {
        eprintln_smooth_budget_recovery_raise_no_step(
            &o.snap,
            o.decode_fps,
            o.rate_opt,
            recover_to,
            o.current_budget_px,
        );
        return;
    }
    eprintln_smooth_budget_recovery_raise(
        &o.snap,
        o.decode_fps,
        o.rate_opt,
        o.decode_px,
        o.current_budget_px,
        recover_to,
    );
    let _ = persist_budget_and_maybe_rebuild_vf(
        player,
        video_pref,
        recover_to,
        "(smooth playback timing quiet — raising ME budget)",
    );
}

fn eprintln_smooth_budget_hold_line(o: &TransportBudgetOutcome) {
    let me_budget_px = o.current_budget_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let hz = budget_signal_hz_for_comparison(o.decode_fps, o.snap.src);
    let src_explain =
        "rolling_strain_recovery<10pct_30ticks strict_overload_tail>40pct_5ticks";
    eprintln!(
        "[rhino] smooth: decision hold signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} overload_window_rate_opt={} overload_window_ready={} recovery_window_rate_opt={} recovery_window_ready={} recovery_streak={}/{}({src_explain}) overload_streak={}/{}(>{} strict_tail strain need {} ticks) allow_raise={} overload_session_block={} process_cpu_share={:?} decode_px²={:?} ME_budget_px²={me_budget_px} denom_hz={:.1}",
        o.snap.src.as_str(),
        o.snap.primary,
        o.snap.mistimed,
        o.snap.vo_drop,
        o.snap.decoder_drop,
        fmt_window_rate(o.rate_opt),
        o.overload_window_ready,
        fmt_window_rate(o.recovery_rate_opt),
        o.recovery_window_ready,
        o.recovery_quiet_streak,
        RECOVERY_FIRE_STREAK_TICKS,
        o.overload_streak,
        OVERLOAD_FIRE_STREAK_TICKS,
        OVERLOAD_STRAIN_GT_FRAC,
        OVERLOAD_FIRE_STREAK_TICKS,
        o.allow_recovery_raise,
        o.recovery_blocked_after_overload_this_open,
        o.process_cpu_frac,
        o.decode_px,
        hz,
    );
}
