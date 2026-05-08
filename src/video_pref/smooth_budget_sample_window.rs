fn trim_smooth_budget_samples(st: &mut SmoothBudgetDecoderState, now: Instant) {
    let cutoff = now - Duration::from_secs(DROP_WINDOW_SECS);
    while st.samples.front().is_some_and(|(t, _)| *t < cutoff) {
        st.samples.pop_front();
    }
}

fn bump_overload_streak_from_strict_rate(st: &mut SmoothBudgetDecoderState, overload_rate_opt: Option<f64>) {
    match overload_rate_opt {
        Some(r) if r > OVERLOAD_STRAIN_GT_FRAC => {
            st.overload_streak = st.overload_streak.saturating_add(1);
        }
        _ => {
            st.overload_streak = 0;
        }
    }
}

fn bump_recovery_quiet_streak(st: &mut SmoothBudgetDecoderState, recovery_rate_opt: Option<f64>) {
    match recovery_rate_opt {
        Some(r) if r < RECOVERY_STRAIN_LT_FRAC => {
            st.recovery_quiet_streak = st.recovery_quiet_streak.saturating_add(1);
        }
        Some(_) => {
            st.recovery_quiet_streak = 0;
        }
        None => {}
    }
}

/// Record **`smooth_budget`** strain sample, trim **≈5 s** deque, return trailing rate and fire flags.
fn sample_window_and_fire_flags(
    st: &mut SmoothBudgetDecoderState,
    cur_count: u64,
    now: Instant,
    fps: f64,
    signal_src: SmoothBudgetSignalSrc,
) -> (
    Option<f64>,
    Option<f64>,
    bool,
    bool,
    u32,
    u32,
    bool,
    bool,
) {
    if let Some(prev) = st.prev_signal_total {
        if cur_count < prev {
            st.samples.clear();
            st.overload_streak = 0;
            st.recovery_quiet_streak = 0;
            reset_smooth_drop_stats_window(st);
        }
    }

    st.samples.push_back((now, cur_count));
    trim_smooth_budget_samples(st, now);

    let overload_rate_opt = overload_rate_from_tail(st, cur_count, now, fps, signal_src);
    let recovery_rate_opt =
        strain_rate_since_deque_front(st, cur_count, now, fps, signal_src, RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS);

    bump_overload_streak_from_strict_rate(st, overload_rate_opt);
    bump_recovery_quiet_streak(st, recovery_rate_opt);

    st.prev_signal_total = Some(cur_count);

    let overload_fire = match overload_rate_opt {
        Some(r) if r > OVERLOAD_STRAIN_GT_FRAC => st.overload_streak >= OVERLOAD_FIRE_STREAK_TICKS,
        _ => false,
    };
    let recover_fire = st.recovery_quiet_streak >= RECOVERY_FIRE_STREAK_TICKS;
    (
        overload_rate_opt,
        recovery_rate_opt,
        overload_fire,
        recover_fire,
        st.overload_streak,
        st.recovery_quiet_streak,
        overload_rate_opt.is_some(),
        recovery_rate_opt.is_some(),
    )
}
