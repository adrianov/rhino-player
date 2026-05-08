// Process CPU sampling and persisted **`video_smooth_max_area`** for the bundled MVTools script.

use std::cell::RefCell;
use std::rc::Rc;

/// Target: average process CPU core-equivalent ≤ this fraction of logical cores relative to the **current**
/// saved ME pixel budget (each overload step scales that budget, not the factory default).
const TARGET_CPU_CORE_FRACTION: f64 = 0.75;
/// Require this many consecutive overloaded transport ticks before shrinking the budget.
const OVERLOAD_STREAK_TICKS: u32 = 2;

#[derive(Default)]
pub(crate) struct SmoothOverloadState {
    probe: SmoothBudgetProbe,
    overload_streak: u32,
}

#[derive(Default)]
struct SmoothBudgetProbe {
    prev_wall: Option<std::time::Instant>,
    prev_cpu_secs: Option<f64>,
}

impl SmoothBudgetProbe {
    /// Process CPU core-equivalent over the last interval: `Δ(user+system CPU seconds) / Δwall`.
    fn tick_busy_cores(&mut self) -> Option<f64> {
        let wall = std::time::Instant::now();
        let cpu = process_cpu_seconds();
        let out = match (self.prev_wall, self.prev_cpu_secs) {
            (Some(pw), Some(pc)) => {
                let dt = wall.duration_since(pw).as_secs_f64().max(1e-6);
                let dc = (cpu - pc).max(0.0);
                Some(dc / dt)
            }
            _ => None,
        };
        self.prev_wall = Some(wall);
        self.prev_cpu_secs = Some(cpu);
        out
    }
}

fn logical_cpu_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|n| u32::try_from(n.get()).unwrap_or(1))
        .unwrap_or(1)
        .max(1)
}

fn process_cpu_seconds() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `getrusage` writes valid fields on success; zero-init is acceptable beforehand.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 {
        return 0.0;
    }
    let u = (ru.ru_utime.tv_sec as f64) + (ru.ru_utime.tv_usec as f64) * 1e-6;
    let s = (ru.ru_stime.tv_sec as f64) + (ru.ru_stime.tv_usec as f64) * 1e-6;
    u + s
}

pub(crate) fn clamp_smooth_area(px: u64) -> u64 {
    px.max(crate::db::MIN_SMOOTH_MAX_AREA)
}

/// `busy_cores` from [SmoothBudgetProbe::tick_busy_cores]. Scales **`current_budget_px`**, not the default
/// 1920×1080 baseline, so sustained overload steps down from whatever is already saved.
pub(crate) fn budget_after_overload(current_budget_px: u64, busy_cores: f64, cpu_cores: u32) -> u64 {
    let base = current_budget_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let cc = f64::from(cpu_cores.max(1));
    let bc = busy_cores.max(1e-9);
    let v = base as f64 * cc * TARGET_CPU_CORE_FRACTION / bc;
    clamp_smooth_area(v.round() as u64)
}

/// Called from the **1 Hz** transport tick (bundled `.vpy` only): shrink **`smooth_max_area`** when the
/// process sustains high CPU, persist, and rebuild **`vf`**.
pub(crate) fn smooth_budget_on_transport_tick(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    pause: bool,
    core_idle: bool,
    overload_state: &RefCell<SmoothOverloadState>,
) {
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
    if !vf_chain_has_vapoursynth(&b.mpv) || !mvtools_vf_eligible(&b.mpv, None) {
        return;
    }
    drop(g);

    let busy_cores = {
        let mut st = overload_state.borrow_mut();
        let Some(bc) = st.probe.tick_busy_cores() else {
            return;
        };
        let cores = logical_cpu_cores();
        let util = bc / f64::from(cores);
        if util > TARGET_CPU_CORE_FRACTION {
            st.overload_streak = st.overload_streak.saturating_add(1);
        } else {
            st.overload_streak = 0;
        }
        if st.overload_streak < OVERLOAD_STREAK_TICKS {
            return;
        }
        st.overload_streak = 0;
        bc
    };

    let cores = logical_cpu_cores();
    let current_budget_px = video_pref.borrow().smooth_max_area;
    let cand = budget_after_overload(current_budget_px, busy_cores, cores);

    let should_apply = {
        let mut vp = video_pref.borrow_mut();
        if cand == vp.smooth_max_area {
            return;
        }
        eprintln!(
            "[rhino] video: smooth_max_area {} → {} px² (process CPU high — lowering ME budget)",
            vp.smooth_max_area, cand
        );
        vp.smooth_max_area = cand;
        crate::db::save_video(&vp);
        true
    };

    if !should_apply {
        return;
    }

    let mut vp = video_pref.borrow_mut();
    let Ok(mut g) = player.try_borrow_mut() else {
        return;
    };
    let Some(b) = g.as_mut() else {
        return;
    };
    let _ = apply_mpv_video(b, &mut vp, None);
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn formula_returns_baseline_at_target_utilization() {
        let cores = 8_u32;
        let busy = f64::from(cores) * TARGET_CPU_CORE_FRACTION;
        assert_eq!(
            budget_after_overload(crate::db::DEFAULT_SMOOTH_MAX_AREA, busy, cores),
            crate::db::DEFAULT_SMOOTH_MAX_AREA
        );
    }

    #[test]
    fn formula_scales_from_current_budget_not_default() {
        let cores = 8_u32;
        let busy = f64::from(cores);
        let half = crate::db::DEFAULT_SMOOTH_MAX_AREA / 2;
        assert_eq!(
            budget_after_overload(half, busy, cores),
            clamp_smooth_area((half as f64 * TARGET_CPU_CORE_FRACTION).round() as u64)
        );
    }
}
