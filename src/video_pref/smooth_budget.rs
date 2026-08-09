// Bundled **ME pixel-area** tuning from mpv presentation / output strain (transport tick ≈ **1 Hz**).
// Overload / recovery persist **`media.smooth_me_budget_px2`** for the open file; **`VideoPrefs.smooth_max_area`**
// stays the Preferences default used as **`resolve_media_smooth_me_budget`** fallback for new paths.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Sliding window length for decoding-stress (**seconds**).
const DROP_WINDOW_SECS: u64 = 3;

/// Overload fires when rolling strain **>** this fraction (**strict tail**, **`OVERLOAD_FIRE_STREAK_TICKS`** successive ticks).
const OVERLOAD_STRAIN_GT_FRAC: f64 = 0.20;

/// Consecutive overload ticks (**~seconds**) before persisting a lower ME budget or busy-system pause.
const OVERLOAD_FIRE_STREAK_TICKS: u32 = 2;

/// **Recovery** rolling tail for **VO** / **decoder** when **`mistimed-frame-count`** is absent — minimum wall span before strain **rates** exist (`overload` shares this gate; samples still trimmed at **[`DROP_WINDOW_SECS`]**).
const RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS: f64 = 1.0;

/// Relaxed-window rolling strain must stay **strictly below** this **fraction** for **`RECOVERY_FIRE_STREAK_TICKS`** successive ticks before ME raise.
const RECOVERY_STRAIN_LT_FRAC: f64 = 0.10;

/// **~300 s** at **`1 Hz`** with **`recovery_rate`** **`<`** **`RECOVERY_STRAIN_LT_FRAC`** before **`recovery_candidate`** raise.
const RECOVERY_FIRE_STREAK_TICKS: u32 = 300;

/// Strain **fraction** = Δtally ÷ (**Δwall × denominator Hz**).
#[must_use]
pub(crate) fn budget_signal_rate_in_window(signal_delta: u64, elapsed_secs: f64, denominator_hz: f64) -> f64 {
    let hz = denominator_hz.clamp(0.05_f64, 960.0);
    let frames = elapsed_secs.max(1e-6) * hz;
    (signal_delta as f64 / frames.max(1.0)).min(10.0)
}

/// `(instant, cumulative **budget signal** tally)` plus optional **`RHINO_SMOOTH_DROP_STATS`** baselines.
#[derive(Default)]
pub(crate) struct SmoothBudgetDecoderState {
    samples: VecDeque<(Instant, u64)>,
    prev_signal_total: Option<u64>,
    recovery_quiet_streak: u32,
    overload_streak: u32,
    /// After a **successful** overload shrink (**smaller** ME px²) on this **`loadfile`** / **`path`**, disallow recovery raises.
    recovery_blocked_after_overload_this_open: bool,
    /// Last **`getrusage`** sample for process CPU-share between transport ticks.
    rusage_cpu_prev: Option<(Instant, u64)>,
    smooth_drop_prev_emit_wall: Option<Instant>,
    smooth_drop_signal_base: u64,
    smooth_drop_mistimed_baseline: Option<u64>,
    smooth_drop_vo_baseline: Option<u64>,
    smooth_drop_decoder_baseline: Option<u64>,
}

include!("smooth_budget_cpu.rs");

include!("smooth_budget_sampling.rs");
include!("smooth_budget_drop_log.rs");

pub(crate) fn clamp_smooth_area(px: u64) -> u64 {
    px.max(crate::db::MIN_SMOOTH_MAX_AREA)
}

/// Rolling-recovery ceiling: decoded width×height when known, else **`DEFAULT_SMOOTH_MAX_AREA`** (fresh installs / unreadable dims).
#[must_use]
pub(crate) fn recovery_ceiling_px(decode_area_px: Option<u64>) -> u64 {
    decode_area_px
        .map(clamp_smooth_area)
        .unwrap_or(crate::db::DEFAULT_SMOOTH_MAX_AREA)
        .max(crate::db::MIN_SMOOTH_MAX_AREA)
}

/// **+10%** step capped at **[`recovery_ceiling_px`]**, **`None`** when already at that ceiling.
#[must_use]
pub(crate) fn recovery_candidate(saved_px: u64, decode_area_px: Option<u64>) -> Option<u64> {
    let base = clamp_smooth_area(saved_px);
    let cap = recovery_ceiling_px(decode_area_px);
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

include!("smooth_budget_decision_log.rs");

/// **−10%** step (integer half-up **`⌊saved×90+50⌋/100`**), at least **`saved−1`**, floored at **`MIN_SMOOTH_MAX_AREA`** — mirrors **[`recovery_candidate`]** step shape.
/// **`strain_rate`** is kept for overload stderr logs only.
#[must_use]
pub(crate) fn budget_after_decoder_overload(current_budget_px: u64, _strain_rate: f64) -> u64 {
    let base = clamp_smooth_area(current_budget_px);
    let floor_px = crate::db::MIN_SMOOTH_MAX_AREA;
    if base <= floor_px {
        return base;
    }
    let scaled = base
        .checked_mul(90)
        .and_then(|x| x.checked_add(50))
        .map(|x| x / 100)
        .unwrap_or(floor_px);
    let shrunk = scaled.min(base.saturating_sub(1));
    clamp_smooth_area(shrunk.max(floor_px))
}

/// Prefer raising only when **`decode_px` exceeds the clamped persisted cap** (same **`decode ≤ cap`** gate as **`bundled_me_vf_out_wh`** before ME downscale).
#[must_use]
pub(crate) fn raised_me_budget_can_reduce_downscale(decode_px: Option<u64>, smooth_max_px: u64) -> bool {
    let cap = smooth_max_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    decode_px.map_or(true, |px| px > cap)
}

include!("smooth_budget_persist.rs");

include!("smooth_budget_sample_window.rs");

include!("smooth_budget_transport_apply.rs");
include!("smooth_budget_transport_tick.rs");

/// Full reset on **`FileLoaded`** / **`path`** so overload / **`getrusage`** baselines belong to **one open media**.
pub(crate) fn smooth_budget_reset_session_on_new_media(cell: &RefCell<SmoothBudgetDecoderState>) {
    cell.replace(SmoothBudgetDecoderState::default());
}

#[cfg(test)]
include!("smooth_budget_tests.rs");
