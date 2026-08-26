const SMOOTH_DROP_STATS_EVERY: Duration = Duration::from_secs(5);

fn smooth_drop_stats_wanted() -> bool {
    std::env::var(crate::paths::RHINO_SMOOTH_DROP_STATS_VAR)
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn reset_smooth_drop_stats_window(st: &mut SmoothBudgetDecoderState) {
    st.smooth_drop_prev_emit_wall = None;
}

fn fmt_delta_opt(cur: Option<u64>, baseline: Option<u64>, tag: &'static str) -> String {
    match (cur, baseline) {
        (Some(c), Some(b)) => format!(" Δ{tag}={}", c.saturating_sub(b)),
        _ => String::new(),
    }
}

/// **`RHINO_SMOOTH_DROP_STATS`**: stderr approx. every **5 s**: **mistimed** / VO / decoder tallies (**`smooth_budget`** signal ladder).
/// Skips **`eprintln`** when overload already shrank ME this open media **and** strict-window strain is **\<** **[`OVERLOAD_STRAIN_GT_FRAC`]** (still advances the stats window so the next line is not a huge catch-up).
fn maybe_emit_smooth_drop_stats_line(
    st: &mut SmoothBudgetDecoderState,
    snap: &SmoothBudgetSignalSnap,
    decode_fps_approx: f64,
    now: Instant,
    recovery_blocked_after_overload_this_open: bool,
    strict_overload_rate_opt: Option<f64>,
) {
    if !smooth_drop_stats_wanted() {
        return;
    }
    let quiet_after_shrink = recovery_blocked_after_overload_this_open
        && !strict_overload_rate_opt.is_some_and(|r| r >= OVERLOAD_STRAIN_GT_FRAC);
    match st.smooth_drop_prev_emit_wall {
        None => start_smooth_drop_stats_window(st, snap, now),
        Some(t0) => advance_smooth_drop_stats_window(
            st,
            snap,
            decode_fps_approx,
            now,
            t0,
            quiet_after_shrink,
        ),
    }
}

/// First observation: record the wall clock and the signal baselines the next line diffs against.
fn start_smooth_drop_stats_window(
    st: &mut SmoothBudgetDecoderState,
    snap: &SmoothBudgetSignalSnap,
    now: Instant,
) {
    st.smooth_drop_prev_emit_wall = Some(now);
    set_smooth_drop_baselines(st, snap);
}

/// Window elapsed: diff tallies against the stored baselines (printed unless quiet), then rebase.
fn advance_smooth_drop_stats_window(
    st: &mut SmoothBudgetDecoderState,
    snap: &SmoothBudgetSignalSnap,
    decode_fps_approx: f64,
    now: Instant,
    t0: Instant,
    quiet_after_shrink: bool,
) {
    let elapsed = now.saturating_duration_since(t0);
    if elapsed < SMOOTH_DROP_STATS_EVERY {
        return;
    }
    let secs = elapsed.as_secs_f64().max(0.001);
    if !quiet_after_shrink {
        print_smooth_drop_stats_line(st, snap, decode_fps_approx, secs);
    }
    st.smooth_drop_prev_emit_wall = Some(now);
    set_smooth_drop_baselines(st, snap);
}

/// Diff tallies against the stored baselines and emit the `[rhino] smooth: stats` line.
fn print_smooth_drop_stats_line(
    st: &SmoothBudgetDecoderState,
    snap: &SmoothBudgetSignalSnap,
    decode_fps_approx: f64,
    secs: f64,
) {
    let denom_hz = budget_signal_hz_for_comparison(decode_fps_approx, snap.src);
    let est_frames = secs * denom_hz;
    let d_sig = snap.primary.saturating_sub(st.smooth_drop_signal_base);
    let sig_pct = budget_signal_rate_in_window(d_sig, secs, denom_hz) * 100.0;
    let dm = fmt_delta_opt(snap.mistimed, st.smooth_drop_mistimed_baseline, "mistimed");
    let dv = fmt_delta_opt(snap.vo_drop, st.smooth_drop_vo_baseline, "vo");
    let dd = fmt_delta_opt(
        snap.decoder_drop,
        st.smooth_drop_decoder_baseline,
        "decoder",
    );
    eprintln!(
        "[rhino] smooth: stats {:.1}s wall signal={} Δsignal={d_sig} total_signal={}{}{}{} ~{sig_pct:.2}% vs budget_denom_hz est_frames={:.0}",
        secs,
        snap.src.as_str(),
        snap.primary,
        dm,
        dv,
        dd,
        est_frames,
    );
}

fn set_smooth_drop_baselines(st: &mut SmoothBudgetDecoderState, snap: &SmoothBudgetSignalSnap) {
    st.smooth_drop_signal_base = snap.primary;
    st.smooth_drop_mistimed_baseline = snap.mistimed;
    st.smooth_drop_vo_baseline = snap.vo_drop;
    st.smooth_drop_decoder_baseline = snap.decoder_drop;
}
