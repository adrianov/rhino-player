fn playback_fps_for_decode_budget(mpv: &Mpv) -> f64 {
    const LO: f64 = 0.05;
    const HI: f64 = 960.0;
    let spd_raw = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let spd = if spd_raw.is_finite() && (0.01..=8.0).contains(&spd_raw) {
        spd_raw.max(LO)
    } else {
        1.0
    };
    let nominal = mpv.get_property::<f64>("container-fps").unwrap_or(0.0);
    let base_fps = if nominal.is_finite() && nominal > LO && nominal < HI {
        nominal * spd
    } else if let Ok(e) = mpv.get_property::<f64>("estimated-vf-fps") {
        if e.is_finite() && e > LO && e < HI {
            e
        } else {
            (24.0_f64 * spd).min(HI)
        }
    } else {
        (24.0_f64 * spd).min(HI)
    };
    base_fps.min(HI)
}

fn decoder_frame_drop_total_u64(mpv: &Mpv) -> Option<u64> {
    let n = mpv.get_property::<i64>("decoder-frame-drop-count").ok()?;
    Some(if n > 0 { n as u64 } else { 0 })
}

fn vo_frame_drop_total_u64(mpv: &Mpv) -> Option<u64> {
    let n = mpv.get_property::<i64>("frame-drop-count").ok()?;
    Some(if n > 0 { n as u64 } else { 0 })
}

fn mistimed_frame_count_u64(mpv: &Mpv) -> Option<u64> {
    mpv.get_property::<i64>("mistimed-frame-count").ok().map(|n| n.max(0) as u64)
}

/// What drives **`smooth_budget`** overload / recovery (first property that **mpv** exposes wins).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SmoothBudgetSignalSrc {
    Mistimed,
    VoDrop,
    DecoderDrop,
}

impl SmoothBudgetSignalSrc {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::Mistimed => "mistimed",
            Self::VoDrop => "vo_drop",
            Self::DecoderDrop => "decoder_drop",
        }
    }
}

/// Compare **`mistimed` / VO** strain against **`≥ ~60 Hz`** presentation cadence (decode-only **24 fps** inflates **`rate`**).
pub(crate) fn budget_signal_hz_for_comparison(decode_fps: f64, src: SmoothBudgetSignalSrc) -> f64 {
    const MIN_VO: f64 = 60.0;
    match src {
        SmoothBudgetSignalSrc::DecoderDrop => decode_fps.clamp(0.1_f64, 960.0),
        SmoothBudgetSignalSrc::Mistimed | SmoothBudgetSignalSrc::VoDrop => decode_fps.max(MIN_VO).clamp(MIN_VO, 960.0),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SmoothBudgetSignalSnap {
    pub primary: u64,
    pub src: SmoothBudgetSignalSrc,
    pub mistimed: Option<u64>,
    pub vo_drop: Option<u64>,
    pub decoder_drop: Option<u64>,
}

/// Prefer **`mistimed-frame-count`** (display / resample cadence mismatch), then **`frame-drop-count`**, then **`decoder-frame-drop-count`**.
pub(crate) fn read_smooth_budget_signal(mpv: &Mpv) -> Option<SmoothBudgetSignalSnap> {
    let mistimed = mistimed_frame_count_u64(mpv);
    let vo_drop = vo_frame_drop_total_u64(mpv);
    let decoder_drop = decoder_frame_drop_total_u64(mpv);

    let (primary, src) = match (mistimed, vo_drop, decoder_drop) {
        (Some(n), _, _) => (n, SmoothBudgetSignalSrc::Mistimed),
        (None, Some(n), _) => (n, SmoothBudgetSignalSrc::VoDrop),
        (None, None, Some(n)) => (n, SmoothBudgetSignalSrc::DecoderDrop),
        (None, None, None) => return None,
    };

    Some(SmoothBudgetSignalSnap {
        primary,
        src,
        mistimed,
        vo_drop,
        decoder_drop,
    })
}

/// Decoded WxH (**`video-params`**, fallback **`width`/`height`**) → pixel area for budget comparison (**`smooth_me_geometry`**).
fn decode_pixel_area_for_me_budget(mpv: &Mpv) -> Option<u64> {
    fn pair_px(mpw: &Mpv, wk: &str, hk: &str) -> Option<u64> {
        let w = mpw.get_property::<i64>(wk).ok()?;
        let h = mpw.get_property::<i64>(hk).ok()?;
        (w > 0 && h > 0).then_some((w as u64).saturating_mul(h as u64))
    }
    pair_px(mpv, "video-params/w", "video-params/h").or_else(|| pair_px(mpv, "width", "height"))
}

fn overload_rate_from_tail(
    st: &SmoothBudgetDecoderState,
    cur_count: u64,
    now: Instant,
    fps: f64,
    signal_src: SmoothBudgetSignalSrc,
) -> Option<f64> {
    // Same minimum wall span as relaxed recovery (`RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS`):
    // overload previously required ~95% of [`DROP_WINDOW_SECS`] (~4.75 s), so `recovery_*`
    // could report extreme strain while `overload_*` stayed `n/a` and `overload_streak` reset
    // every tick; deque trimming still caps the trailing tail at [`DROP_WINDOW_SECS`].
    strain_rate_since_deque_front(
        st,
        cur_count,
        now,
        fps,
        signal_src,
        RECOVERY_STRAIN_TAIL_MIN_ELAPSED_SECS,
    )
}

#[must_use]
fn strain_rate_since_deque_front(
    st: &SmoothBudgetDecoderState,
    cur_count: u64,
    now: Instant,
    fps: f64,
    signal_src: SmoothBudgetSignalSrc,
    min_elapsed_wall_secs: f64,
) -> Option<f64> {
    let (t_old, c_old) = *st.samples.front()?;
    let elapsed = now.duration_since(t_old).as_secs_f64();
    if elapsed < min_elapsed_wall_secs {
        return None;
    }
    let hz = budget_signal_hz_for_comparison(fps, signal_src);
    Some(budget_signal_rate_in_window(cur_count.saturating_sub(c_old), elapsed, hz))
}
