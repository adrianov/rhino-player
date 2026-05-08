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
