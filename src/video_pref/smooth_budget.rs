// Bundled **`video_smooth_max_area`** tuning from mpv presentation / output strain properties (transport tick ≈ **1 Hz**).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Sliding window length for decoding-stress (**seconds**).
const DROP_WINDOW_SECS: u64 = 5;

/// Overload fires when rolling strain **>** this fraction (**strict tail**, **`OVERLOAD_FIRE_STREAK_TICKS`** successive ticks).
const OVERLOAD_STRAIN_GT_FRAC: f64 = 0.20;

/// Consecutive overload ticks (**~seconds**) before persisting a lower ME budget.
const OVERLOAD_FIRE_STREAK_TICKS: u32 = 5;

/// **Recovery** rolling tail for **VO** / **decoder** when **`mistimed-frame-count`** is absent — minimum wall span before strain **rates** exist (`overload` shares this gate; samples still trimmed at **[`DROP_WINDOW_SECS`]**).
const RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS: f64 = 2.1;

/// Relaxed-window rolling strain must stay **strictly below** this **fraction** for **`RECOVERY_FIRE_STREAK_TICKS`** successive ticks before ME raise.
const RECOVERY_STRAIN_LT_FRAC: f64 = 0.10;

/// **~30 s** at **`1 Hz`** with **`recovery_rate`** **`<`** **`RECOVERY_STRAIN_LT_FRAC`** before **`recovery_candidate`** raise.
const RECOVERY_FIRE_STREAK_TICKS: u32 = 30;

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

/// Persist a new **`video_smooth_max_area`** and run **`apply_mpv_video`** so **`RHINO_SMOOTH_MAX_AREA`** and the bundled
/// graph match SQLite.
#[must_use]
fn persist_budget_and_maybe_rebuild_vf(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    new_budget_px: u64,
    stderr_reason_suffix: &'static str,
) -> bool {
    {
        let mut vp = video_pref.borrow_mut();
        if new_budget_px == vp.smooth_max_area {
            eprintln!(
                "[rhino] smooth: persist_skip ME_budget_px² unchanged {} ({stderr_reason_suffix})",
                vp.smooth_max_area
            );
            return false;
        }
        eprintln!(
            "[rhino] video: smooth_max_area {} → {} px² {}",
            vp.smooth_max_area, new_budget_px, stderr_reason_suffix
        );
        vp.smooth_max_area = new_budget_px;
        crate::db::save_video(&vp);
    }

    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            if let Some(p) = crate::media_probe::local_file_from_mpv(&b.mpv) {
                crate::db::media_save_smooth_me_budget(&p, new_budget_px);
            }
        }
    }

    let mut vp = video_pref.borrow_mut();
    let Ok(mut g) = player.try_borrow_mut() else {
        return true;
    };
    let Some(b) = g.as_mut() else {
        return true;
    };

    let _ = apply_mpv_video(b, &mut vp, None);
    true
}

include!("smooth_budget_sample_window.rs");

/// **`1 Hz`** transport tick (**bundled** `.vpy` only; **caller** skips ticks while the playback shell window is inactive / unmapped—see **`transport_events`**): tighten ME budget when the **playback smoothness strain tally**
/// shows **>** **`OVERLOAD_STRAIN_GT_FRAC`** strict rolling strain **five** successive ticks; relax when relaxed-window strain **\<** **`RECOVERY_STRAIN_LT_FRAC`** **thirty** successive ticks.
pub(crate) fn smooth_budget_on_transport_tick(
    player: &Rc<RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    video_pref: &Rc<RefCell<crate::db::VideoPrefs>>,
    pause: bool,
    core_idle: bool,
    state_cell: &RefCell<SmoothBudgetDecoderState>,
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
    let Some(snap) = read_smooth_budget_signal(&b.mpv) else {
        return;
    };
    let cur_count = snap.primary;
    let fps = playback_fps_for_decode_budget(&b.mpv);
    let decode_px = decode_pixel_area_for_me_budget(&b.mpv);
    let current_budget_px = {
        let vp = video_pref.borrow();
        effective_smooth_me_budget_px(&b.mpv, &vp)
    };
    drop(g);
    let now = Instant::now();
    let allow_recovery_raise = raised_me_budget_can_reduce_downscale(decode_px, current_budget_px);
    let recovery_blocked_after_overload_snapshot =
        state_cell.borrow().recovery_blocked_after_overload_this_open;
    let (
        rate_opt,
        recovery_rate_opt,
        overload_fire,
        recover_fire,
        overload_streak,
        recovery_quiet_streak,
        overload_window_ready,
        recovery_window_ready,
    ) = {
        let mut st = state_cell.borrow_mut();
        let out = sample_window_and_fire_flags(&mut st, cur_count, now, fps, snap.src);
        maybe_emit_smooth_drop_stats_line(
            &mut st,
            &snap,
            fps,
            now,
            recovery_blocked_after_overload_snapshot,
            out.0,
        );
        out
    };

    let recovery_blocked_after_overload_this_open =
        state_cell.borrow().recovery_blocked_after_overload_this_open;
    let process_cpu_frac = {
        let mut st = state_cell.borrow_mut();
        smooth_budget_refresh_process_cpu_frac(&mut st)
    };

    let o = TransportBudgetOutcome {
        current_budget_px,
        cur_count,
        now,
        rate_opt,
        recovery_rate_opt,
        overload_fire,
        recover_fire,
        allow_recovery_raise,
        recovery_blocked_after_overload_this_open,
        process_cpu_frac,
        snap,
        decode_fps: fps,
        decode_px,
        overload_streak,
        recovery_quiet_streak,
        overload_window_ready,
        recovery_window_ready,
    };

    apply_budget_actions_after_sample(player, video_pref, state_cell, &o);
}

include!("smooth_budget_transport_apply.rs");

/// Full reset on **`FileLoaded`** / **`path`** so overload / **`getrusage`** baselines belong to **one open media**.
pub(crate) fn smooth_budget_reset_session_on_new_media(cell: &RefCell<SmoothBudgetDecoderState>) {
    cell.replace(SmoothBudgetDecoderState::default());
}

#[cfg(test)]
include!("smooth_budget_tests.rs");
