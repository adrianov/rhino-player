// Process CPU sampling and persisted **`video_smooth_max_area`** for the bundled MVTools script.

use std::cell::RefCell;
use std::rc::Rc;

/// Target: average process CPU core-equivalent ≤ this fraction of logical cores relative to the **current**
/// saved ME pixel budget (each overload step scales that budget, not the factory default).
const TARGET_CPU_CORE_FRACTION: f64 = 0.75;
/// Require this many consecutive overloaded transport ticks before shrinking the budget.
const OVERLOAD_STREAK_TICKS: u32 = 2;

/// Recover when utilization stays strictly **below** this fraction of logical cores (see [`SmoothBudgetProbe::tick_busy_cores`]).
const UNDERUTIL_CPU_CORE_FRACTION: f64 = 0.50;
/// Require this many consecutive underutil ticks before raising the ME budget (**1 Hz** tick ≈ seconds).
const UNDERUTIL_STREAK_TICKS: u32 = 30;

#[derive(Default)]
pub(crate) struct SmoothOverloadState {
    probe: SmoothBudgetProbe,
    overload_streak: u32,
    underutil_streak: u32,
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

/// **+10%** step capped at **`DEFAULT_SMOOTH_MAX_AREA`**, **`None`** when already at nominal default or invalid.
#[must_use]
pub(crate) fn recovery_candidate(saved_px: u64) -> Option<u64> {
    let base = clamp_smooth_area(saved_px);
    let cap = crate::db::DEFAULT_SMOOTH_MAX_AREA;
    if base >= cap {
        return None;
    }
    let scaled = base
        .checked_mul(110)
        .and_then(|x| x.checked_add(50))
        .map(|x| x / 100)
        .unwrap_or(u64::MAX);
    let bumped = scaled.max(base.saturating_add(1));
    let limited = bumped.min(cap);
    Some(clamp_smooth_area(limited))
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

/// Persist a new **`video_smooth_max_area`** and run **`apply_mpv_video`** so **`RHINO_SMOOTH_MAX_AREA`** and the bundled
/// VapourSynth graph match SQLite — a warm **mpv** interpreter does **not** pick up env/token-only tweaks.
fn persist_budget_and_maybe_rebuild_vf(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    new_budget_px: u64,
    stderr_reason_suffix: &'static str,
) {
    {
        let mut vp = video_pref.borrow_mut();
        if new_budget_px == vp.smooth_max_area {
            return;
        }
        eprintln!(
            "[rhino] video: smooth_max_area {} → {} px² {}",
            vp.smooth_max_area, new_budget_px, stderr_reason_suffix
        );
        vp.smooth_max_area = new_budget_px;
        crate::db::save_video(&vp);
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

/// Called from the **1 Hz** transport tick (bundled `.vpy` only): adjust **`smooth_max_area`** on sustained
/// high (**shrink**) or low (**grow**) CPU **load**, then **`apply_mpv_video`** so the warm VapourSynth instance
/// observes the new ME budget (env + interpreter both).
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

    let current_budget_px = video_pref.borrow().smooth_max_area;

    let (busy_cores, cores, overload_fire, recover_fire) = {
        let mut st = overload_state.borrow_mut();
        let Some(bc) = st.probe.tick_busy_cores() else {
            return;
        };
        let cores = logical_cpu_cores();
        let util = bc / f64::from(cores);
        match util {
            u if u > TARGET_CPU_CORE_FRACTION => {
                st.overload_streak = st.overload_streak.saturating_add(1);
                st.underutil_streak = 0;
            }
            u if u < UNDERUTIL_CPU_CORE_FRACTION => {
                st.underutil_streak = st.underutil_streak.saturating_add(1);
                st.overload_streak = 0;
            }
            _ => {
                st.overload_streak = 0;
                st.underutil_streak = 0;
            }
        }

        let mut overload_fire = false;
        if st.overload_streak >= OVERLOAD_STREAK_TICKS {
            st.overload_streak = 0;
            overload_fire = true;
        }

        let mut recover_fire = false;
        if st.underutil_streak >= UNDERUTIL_STREAK_TICKS {
            st.underutil_streak = 0;
            recover_fire = true;
        }

        (bc, cores, overload_fire, recover_fire)
    };

    if overload_fire {
        let cand = budget_after_overload(current_budget_px, busy_cores, cores);
        persist_budget_and_maybe_rebuild_vf(
            player,
            video_pref,
            cand,
            "(process CPU high — lowering ME budget)",
        );
        return;
    }

    if recover_fire {
        if let Some(recover_to) = recovery_candidate(current_budget_px) {
            if recover_to > current_budget_px {
                persist_budget_and_maybe_rebuild_vf(
                    player,
                    video_pref,
                    recover_to,
                    "(process CPU low — raising ME budget)",
                );
            }
        }
    }
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

    #[test]
    fn recovery_raises_by_ten_percent_and_caps_default() {
        assert_eq!(
            recovery_candidate(500_000_u64),
            Some(clamp_smooth_area(550_000_u64))
        );
        let just_below_default = crate::db::DEFAULT_SMOOTH_MAX_AREA - 800;
        assert_eq!(recovery_candidate(just_below_default), Some(crate::db::DEFAULT_SMOOTH_MAX_AREA));
    }

    #[test]
    fn recovery_unknown_at_nominal_ceiling() {
        assert_eq!(recovery_candidate(crate::db::DEFAULT_SMOOTH_MAX_AREA), None);
    }
}
