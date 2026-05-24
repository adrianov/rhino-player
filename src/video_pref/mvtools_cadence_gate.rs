// Smooth cadence gate: disc fps lock, interleaved display-resample vs MVTools.

/// After `loadfile`, `estimated-vf-fps` can still reflect the previous clip longer than one idle tick.
/// Pairing that stale value with the new file’s `container-fps` (often ~24) incorrectly triggers the
/// NTSC film tie-break. Drop `estimated-vf-fps` for the first `FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE`
/// `source_fps_from_mpv` reads after `path` changes (several rebuilds / resyncs can run before mpv updates).
const FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE: u32 = 6;
/// Consecutive plausible cadence reads before MVTools on a disc (interleaved titles need settle time).
const CADENCE_STABLE_READS: u32 = 3;
const CADENCE_JUMP_FRAC: f64 = 0.12;

#[derive(Debug, Clone, Default)]
struct FpsPickGateState {
    last_path: Option<String>,
    ignore_est_left: u32,
    /// Optical disc: ignore wild `estimated-vf-fps` once a plausible container rate is known.
    locked_disc_fps: Option<f64>,
    /// Interleaved / VFR: use mpv **display-resample** instead of VapourSynth (no cadence rebuild loop).
    interleaved_smooth: bool,
    stable_streak: u32,
    last_stable_fps: Option<f64>,
}

/// Ignore `estimated-vf-fps` when it would describe vf **output** (~60 Hz) or unstable disc demux.
fn ignore_est_for_source_pick(
    path: Option<&str>,
    mpv: &libmpv2::Mpv,
    shell: Option<&std::path::Path>,
) -> bool {
    path.is_some_and(mpv_path_is_disc)
        || path_str_is_dvd_vob(path)
        || shell_path_is_dvd_vob(shell)
        || vf_chain_has_vapoursynth(mpv)
}

fn is_plausible_broadcast_fps(f: f64) -> bool {
    const RATES: [f64; 6] = [
        24000.0 / 1001.0,
        24.0,
        25.0,
        30000.0 / 1001.0,
        29.97,
        30.0,
    ];
    RATES.iter().any(|r| (f - r).abs() < 0.2)
}

fn stabilize_disc_source_fps(
    path: Option<&str>,
    picked: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    if !path.is_some_and(mpv_path_is_disc) {
        gate.locked_disc_fps = None;
        return picked;
    }
    match picked {
        Some(f) if is_plausible_broadcast_fps(f) => {
            gate.locked_disc_fps = Some(f);
            Some(f)
        }
        Some(_) => gate.locked_disc_fps,
        None => gate.locked_disc_fps,
    }
}

static FPS_PICK_GATE: Mutex<FpsPickGateState> = Mutex::new(FpsPickGateState {
    last_path: None,
    ignore_est_left: 0,
    locked_disc_fps: None,
    interleaved_smooth: false,
    stable_streak: 0,
    last_stable_fps: None,
});

/// After a **seek**, mpv cadence readings fluctuate on interleaved discs — stay on display-resample until stable.
fn mark_smooth_cadence_unstable_after_seek() {
    let mut g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    g.interleaved_smooth = true;
    g.stable_streak = 0;
    g.last_stable_fps = None;
    g.locked_disc_fps = None;
}

fn smooth_cadence_unstable_target(mpv: &libmpv2::Mpv) -> bool {
    let path = mpv
        .get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty());
    path.as_deref().is_some_and(mpv_path_is_disc)
        || path_str_is_dvd_vob(path.as_deref())
        || crate::media_probe::local_file_from_mpv(mpv)
            .is_some_and(|p| crate::video_ext::is_dvd_vob_path(&p))
}

/// Seek on optical-disc media: prefer display-resample until cadence stabilizes.
pub(crate) fn mark_smooth_cadence_unstable_after_seek_if_disc(mpv: &libmpv2::Mpv) {
    if smooth_cadence_unstable_target(mpv) {
        mark_smooth_cadence_unstable_after_seek();
    }
}

/// True when Smooth 60 should use mpv **display-resample** only (no VapourSynth / cadence rebuild).
pub(crate) fn smooth_prefers_display_resample(
    mpv: &libmpv2::Mpv,
    shell_disc: Option<&std::path::Path>,
    shell_media: Option<&std::path::Path>,
) -> bool {
    let path_now = mpv
        .get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    if shell_path_is_dvd_vob(shell_media) || path_str_is_dvd_vob(path_now.as_deref()) {
        return g.interleaved_smooth;
    }
    let disc = path_now
        .as_deref()
        .is_some_and(mpv_path_is_disc)
        || shell_disc.is_some_and(crate::video_ext::is_optical_disc_path);
    if g.interleaved_smooth {
        return true;
    }
    disc && g.stable_streak < CADENCE_STABLE_READS
}

fn cadence_rates_jump(prev: f64, f: f64) -> bool {
    let jump = (f - prev).abs();
    let rel = (f / prev - 1.0).abs();
    rel > CADENCE_JUMP_FRAC || jump > (prev * CADENCE_JUMP_FRAC).max(1.5)
}

fn note_plausible_cadence(f: f64, gate: &mut FpsPickGateState, disc: bool) -> bool {
    let mut cadence_jump = false;
    if let Some(prev) = gate.last_stable_fps {
        if disc && cadence_rates_jump(prev, f) {
            gate.interleaved_smooth = true;
            gate.stable_streak = 0;
            cadence_jump = true;
        } else if (f - prev).abs() < 0.03 {
            gate.stable_streak = gate.stable_streak.saturating_add(1);
        } else {
            gate.stable_streak = 1;
        }
    } else {
        gate.stable_streak = 1;
    }
    gate.last_stable_fps = Some(f);
    if !cadence_jump && gate.stable_streak >= CADENCE_STABLE_READS {
        gate.interleaved_smooth = false;
    }
    cadence_jump
}

fn update_interleaved_cadence_gate(
    path: Option<&str>,
    picked: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let disc = path.is_some_and(mpv_path_is_disc);
    match picked {
        None => {
            // Disc demux often omits cadence mid-title; local files may omit reads while vf runs.
            if disc {
                gate.interleaved_smooth = true;
                gate.stable_streak = 0;
            }
        }
        Some(f) if !is_plausible_broadcast_fps(f) => {
            if disc {
                gate.interleaved_smooth = true;
                gate.stable_streak = 0;
            }
            gate.last_stable_fps = Some(f);
        }
        Some(f) => {
            let _ = note_plausible_cadence(f, gate, disc);
        }
    }
    picked.or(gate.locked_disc_fps)
}

fn mask_est_for_path_change_with_state(
    path_now: Option<String>,
    est: Option<f64>,
    gate: &mut FpsPickGateState,
    shell: Option<&std::path::Path>,
) -> Option<f64> {
    let path_changed = gate.last_path != path_now;
    if path_changed {
        gate.last_path.clone_from(&path_now);
        gate.ignore_est_left = FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE;
        gate.locked_disc_fps = None;
        gate.stable_streak = 0;
        gate.last_stable_fps = None;
        let dvd_vob =
            path_str_is_dvd_vob(path_now.as_deref()) || shell_path_is_dvd_vob(shell);
        gate.interleaved_smooth = path_now
            .as_deref()
            .is_some_and(mpv_path_is_disc)
            && !dvd_vob;
    }
    if gate.ignore_est_left > 0 {
        gate.ignore_est_left -= 1;
        None
    } else {
        est
    }
}
