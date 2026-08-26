/// Shared `Option` → log-text helpers for `[rhino] video:` property readouts (`?` when missing).
pub(crate) fn fmt_opt_str(o: Option<String>, missing: &str) -> String {
    o.unwrap_or_else(|| missing.to_string())
}

pub(crate) fn fmt_bool_opt(x: Option<bool>) -> String {
    x.map(|b| b.to_string()).unwrap_or_else(|| "?".into())
}

pub(crate) fn fmt_secs_opt(x: Option<f64>, precision: usize) -> String {
    x.map(|v| format!("{v:.precision$}"))
        .unwrap_or_else(|| "?".into())
}

/// Always-on (throttled) A/V offset readout while the smooth **`vf`** is active, so lip-sync drift
/// is visible on plain `cargo run` without env flags. mpv **`avsync`** is audio-minus-video seconds.
pub(crate) fn log_smooth_avsync(mpv: &libmpv2::Mpv) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if !vf_chain_has_vapoursynth(mpv) || mpv.get_property::<bool>("pause").unwrap_or(true) {
        return;
    }
    if avsync_log_throttled(&LAST, Duration::from_secs(2)) {
        return;
    }
    let (avsync, pos, vf_fps, display_fps) = read_avsync_snapshot(mpv);
    let tag = avsync_drift_tag(avsync);
    eprintln!(
        "[rhino] video: avsync {tag} a-v={} time-pos={} vf-fps={} display-fps={}",
        avsync
            .map(|a| format!("{a:+.3}s"))
            .unwrap_or_else(|| "?".into()),
        fmt_secs_opt(pos, 2),
        fmt_secs_opt(vf_fps, 2),
        fmt_secs_opt(display_fps, 2),
    );
}

/// mpv A/V readout: offset, playhead, filter-output fps, display fps.
fn read_avsync_snapshot(
    mpv: &libmpv2::Mpv,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    (
        mpv.get_property::<f64>("avsync").ok(),
        mpv.get_property::<f64>("time-pos").ok(),
        mpv.get_property::<f64>("estimated-vf-fps").ok(),
        mpv.get_property::<f64>("display-fps").ok(),
    )
}

/// Records `now` and returns `true` when the last emit is younger than [min_interval].
fn avsync_log_throttled(
    last: &std::sync::Mutex<Option<std::time::Instant>>,
    min_interval: std::time::Duration,
) -> bool {
    let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed() < min_interval) {
        return true;
    }
    *last = Some(std::time::Instant::now());
    false
}

/// `DRIFT` when |a-v| exceeds ~80 ms (lip sync visibly off), else `ok`.
fn avsync_drift_tag(avsync: Option<f64>) -> &'static str {
    match avsync {
        Some(a) if a.abs() > 0.08 => "DRIFT",
        _ => "ok",
    }
}

/// Pause across a **`vf`** swap when playback was running; paired with [schedule_vf_playhead_resync].
#[derive(Clone, Copy)]
pub(crate) struct VfAvSnap {
    pub(crate) was_playing: bool,
    /// True when this snap called [screen_blackout::begin_tech_hold].
    tech_hold: bool,
}

/// When [pause_if_playing] is false (first **`vf add`** after open), record play state but do not pause.
pub(crate) fn vf_swap_snap(mpv: &libmpv2::Mpv, pause_if_playing: bool) -> VfAvSnap {
    let was_playing = !mpv.get_property::<bool>("pause").unwrap_or(true);
    let mut tech_hold = false;
    if pause_if_playing && was_playing {
        crate::screen_blackout::begin_tech_hold();
        tech_hold = true;
        let _ = mpv.set_property("pause", true);
    }
    VfAvSnap {
        was_playing,
        tech_hold,
    }
}

pub(crate) fn vf_swap_unpause(mpv: &libmpv2::Mpv, snap: &VfAvSnap) {
    if snap.was_playing {
        let _ = mpv.set_property("pause", false);
    }
    if snap.tech_hold {
        crate::screen_blackout::end_tech_hold();
    }
}

pub(crate) fn vf_av_ping_render(bundle: Option<&crate::mpv_embed::MpvBundle>) {
    #[cfg(not(target_os = "macos"))]
    if let Some(b) = bundle {
        b.linux_ping_render_context();
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = bundle {
        b.macos_ping_render_context();
        b.macos_mark_display_pending();
    }
}
