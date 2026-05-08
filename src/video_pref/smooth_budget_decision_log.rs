fn fmt_window_rate(rate: Option<f64>) -> String {
    rate.filter(|v| v.is_finite())
        .map(|v| format!("{:.4} (~{:.2}% strain)", v, v * 100.0))
        .unwrap_or_else(|| String::from("n/a"))
}

fn eprintln_smooth_budget_overload(
    snap: &SmoothBudgetSignalSnap,
    decode_fps: f64,
    rate: f64,
    from_px: u64,
    to_px: u64,
) {
    let hz = budget_signal_hz_for_comparison(decode_fps, snap.src);
    eprintln!(
        "[rhino] smooth: decision overload signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} window_rate={:.4} (~{:.2}% strain vs {:.1} Hz denom) ME_budget_px² {} → {}",
        snap.src.as_str(),
        snap.primary,
        snap.mistimed,
        snap.vo_drop,
        snap.decoder_drop,
        rate,
        rate * 100.0,
        hz,
        from_px,
        to_px,
    );
}

fn eprintln_smooth_budget_recovery_raise(
    snap: &SmoothBudgetSignalSnap,
    decode_fps: f64,
    rate_opt: Option<f64>,
    decode_px: Option<u64>,
    from_px: u64,
    to_px: u64,
) {
    let hz = budget_signal_hz_for_comparison(decode_fps, snap.src);
    eprintln!(
        "[rhino] smooth: decision raise signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} after_flat_ticks≥{} quiet window_rate_opt={} decode_px²={:?} {:.1} Hz denom ME_budget_px² {} → {}",
        snap.src.as_str(),
        snap.primary,
        snap.mistimed,
        snap.vo_drop,
        snap.decoder_drop,
        RECOVERY_FIRE_STREAK_TICKS,
        fmt_window_rate(rate_opt),
        decode_px,
        hz,
        from_px,
        to_px,
    );
}

fn eprintln_smooth_budget_recovery_skip_decode_fits_cap(
    snap: &SmoothBudgetSignalSnap,
    decode_fps: f64,
    rate_opt: Option<f64>,
    decode_px: Option<u64>,
    me_cap_px: u64,
) {
    let hz = budget_signal_hz_for_comparison(decode_fps, snap.src);
    eprintln!(
        "[rhino] smooth: decision raise_skipped decode_area_fits_ME_cap signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} after_flat_ticks≥{} quiet window_rate_opt={} decode_px²={:?} ME_cap_px²={} {:.1} Hz denom",
        snap.src.as_str(),
        snap.primary,
        snap.mistimed,
        snap.vo_drop,
        snap.decoder_drop,
        RECOVERY_FIRE_STREAK_TICKS,
        fmt_window_rate(rate_opt),
        decode_px,
        me_cap_px,
        hz,
    );
}

fn eprintln_smooth_budget_recovery_raise_no_step(
    snap: &SmoothBudgetSignalSnap,
    decode_fps: f64,
    rate_opt: Option<f64>,
    recover_to: u64,
    current_px: u64,
) {
    let hz = budget_signal_hz_for_comparison(decode_fps, snap.src);
    eprintln!(
        "[rhino] smooth: decision raise_skipped recovery_candidate_px²≤current signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} window_rate_opt={} recovery_candidate_px²={} ME_budget_px²={} {:.1} Hz denom",
        snap.src.as_str(),
        snap.primary,
        snap.mistimed,
        snap.vo_drop,
        snap.decoder_drop,
        fmt_window_rate(rate_opt),
        recover_to,
        current_px,
        hz,
    );
}

fn eprintln_smooth_budget_recovery_at_default_ceiling(snap: &SmoothBudgetSignalSnap, me_budget_px: u64) {
    eprintln!(
        "[rhino] smooth: decision raise_skipped at_default_ME_ceiling signal={} primary_total={} mistimed={:?} vo_drop={:?} decoder_drop={:?} ME_budget_px²={} (recovery_candidate exhausted)",
        snap.src.as_str(),
        snap.primary,
        snap.mistimed,
        snap.vo_drop,
        snap.decoder_drop,
        me_budget_px,
    );
}
