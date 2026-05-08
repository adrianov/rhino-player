/// Block recovery raises when this process averaged **>** this share of **all logical processors**
/// over the last transport interval (**`getrusage`** CPU time ÷ wall × **`available_parallelism`**).
const RECOVER_CPU_SHARE_HARD_MAX_FRAC: f64 = 0.75;

#[cfg(unix)]
fn process_rusage_usec() -> Option<u64> {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 {
        return None;
    }
    Some(
        timeval_usec(&ru.ru_utime).saturating_add(timeval_usec(&ru.ru_stime)),
    )
}

#[cfg(unix)]
fn timeval_usec(tv: &libc::timeval) -> u64 {
    (tv.tv_sec as u64)
        .saturating_mul(1_000_000)
        .saturating_add(tv.tv_usec as u64)
}

#[cfg(not(unix))]
fn process_rusage_usec() -> Option<u64> {
    None
}

/// Returns **Some(share)** when a full interval exists; **None** on first sample or missing OS support.
/// **Share** may exceed **1.0** when many threads run in parallel.
fn smooth_budget_refresh_process_cpu_frac(st: &mut SmoothBudgetDecoderState) -> Option<f64> {
    let now = Instant::now();
    let cur = process_rusage_usec()?;
    match st.rusage_cpu_prev {
        None => {
            st.rusage_cpu_prev = Some((now, cur));
            None
        }
        Some((t0, u0)) => {
            let dt = now.saturating_duration_since(t0).as_secs_f64().max(1e-3);
            let du = cur.saturating_sub(u0);
            st.rusage_cpu_prev = Some((now, cur));
            let n = std::thread::available_parallelism()
                .map(|x| x.get() as f64)
                .unwrap_or(1.0)
                .max(1.0);
            let machine_usec = dt * n * 1_000_000.0;
            Some((du as f64) / machine_usec)
        }
    }
}
