// FPS-pick gate state machine: path-change est masking, disc fps lock, interleaved cadence tracking.

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
    const RATES: [f64; 6] = [24000.0 / 1001.0, 24.0, 25.0, 30000.0 / 1001.0, 29.97, 30.0];
    RATES.iter().any(|r| (f - r).abs() < 0.2)
}

fn optical_disc_cadence_context(path: Option<&str>, shell: Option<&std::path::Path>) -> bool {
    path.is_some_and(mpv_path_is_disc) || shell.is_some_and(crate::video_ext::is_optical_disc_path)
}

fn stabilize_disc_source_fps(
    path: Option<&str>,
    shell: Option<&std::path::Path>,
    picked: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    if !optical_disc_cadence_context(path, shell) {
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
    shell: Option<&std::path::Path>,
    picked: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let disc = optical_disc_cadence_context(path, shell);
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
            let jump = note_plausible_cadence(f, gate, disc);
            if disc && !jump && is_plausible_broadcast_fps(f) {
                gate.interleaved_smooth = false;
            }
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
        let dvd_vob = path_str_is_dvd_vob(path_now.as_deref()) || shell_path_is_dvd_vob(shell);
        gate.interleaved_smooth = path_now.as_deref().is_some_and(mpv_path_is_disc) && !dvd_vob;
    }
    if gate.ignore_est_left > 0 {
        gate.ignore_est_left -= 1;
        None
    } else {
        est
    }
}
