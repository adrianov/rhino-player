struct TransportBudgetOutcome {
    current_budget_px: u64,
    cur_count: u64,
    now: Instant,
    rate_opt: Option<f64>,
    overload_fire: bool,
    recover_fire: bool,
    allow_recovery_raise: bool,
    recovery_blocked_after_overload_this_open: bool,
    process_cpu_frac: Option<f64>,
    snap: SmoothBudgetSignalSnap,
    decode_fps: f64,
    decode_px: Option<u64>,
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
                    "[rhino] smooth: decision hold anomaly overload_fire_but_no_strain_gt_overload_frac rate_opt {:?}",
                    o.rate_opt,
                );
            }
        }
        return;
    }

    if o.recover_fire {
        state_cell.borrow_mut().recovery_quiet_streak = 0;
        maybe_handle_recovery_raise(player, video_pref, o);
    }
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
        return;
    }
    if o.recovery_blocked_after_overload_this_open {
        return;
    }
    if o.process_cpu_frac.is_some_and(|f| f > RECOVER_CPU_SHARE_HARD_MAX_FRAC) {
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
        return;
    };
    if recover_to <= o.current_budget_px {
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

