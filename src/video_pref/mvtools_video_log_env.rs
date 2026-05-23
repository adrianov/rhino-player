// RHINO_VIDEO_LOG, libmvtools resolution into RHINO_MVTOOLS_LIB, and RHINO_SOURCE_FPS from mpv.

use std::sync::Mutex;

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
    /// Blu-ray / AVCHD: ignore wild `estimated-vf-fps` once a plausible container rate is known.
    locked_disc_fps: Option<f64>,
    /// Interleaved / VFR: use mpv **display-resample** instead of VapourSynth (no cadence rebuild loop).
    interleaved_smooth: bool,
    stable_streak: u32,
    last_stable_fps: Option<f64>,
}

pub(crate) fn mpv_path_is_disc(path: &str) -> bool {
    let p = path.trim().to_ascii_lowercase();
    p.starts_with("bd://") || p.starts_with("bluray://")
}

fn mpv_has_vapoursynth_vf(mpv: &libmpv2::Mpv) -> bool {
    mpv.get_property::<String>("vf")
        .map(|s| s.to_ascii_lowercase().contains("vapoursynth"))
        .unwrap_or(false)
}

/// Ignore `estimated-vf-fps` when it would describe vf **output** (~60 Hz) or unstable disc demux.
fn ignore_est_for_source_pick(path: Option<&str>, mpv: &libmpv2::Mpv) -> bool {
    path.is_some_and(mpv_path_is_disc) || mpv_has_vapoursynth_vf(mpv)
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

/// After a **seek**, mpv cadence readings fluctuate on interleaved Blu-ray — stay on display-resample until stable.
pub(crate) fn mark_smooth_cadence_unstable_after_seek() {
    let mut g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    g.interleaved_smooth = true;
    g.stable_streak = 0;
    g.last_stable_fps = None;
    g.locked_disc_fps = None;
}

/// True when Smooth 60 should use mpv **display-resample** only (no VapourSynth / cadence rebuild).
pub(crate) fn smooth_prefers_display_resample(
    mpv: &libmpv2::Mpv,
    shell_disc: Option<&std::path::Path>,
) -> bool {
    let path_now = mpv
        .get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let disc = path_now
        .as_deref()
        .is_some_and(mpv_path_is_disc)
        || shell_disc.is_some_and(crate::video_ext::is_bluray_disc_path);
    let g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
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

fn note_plausible_cadence(f: f64, gate: &mut FpsPickGateState) -> bool {
    let mut cadence_jump = false;
    if let Some(prev) = gate.last_stable_fps {
        if cadence_rates_jump(prev, f) {
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

pub(super) fn update_interleaved_cadence_gate(
    path: Option<&str>,
    picked: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let disc = path.is_some_and(mpv_path_is_disc);
    match picked {
        None => {
            if disc || gate.last_stable_fps.is_some() {
                gate.interleaved_smooth = true;
            }
            gate.stable_streak = 0;
        }
        Some(f) if !is_plausible_broadcast_fps(f) => {
            gate.interleaved_smooth = true;
            gate.stable_streak = 0;
            gate.last_stable_fps = Some(f);
        }
        Some(f) => {
            let _ = note_plausible_cadence(f, gate);
        }
    }
    picked.or(gate.locked_disc_fps)
}

fn mask_est_for_path_change_with_state(
    path_now: Option<String>,
    est: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let path_changed = gate.last_path != path_now;
    if path_changed {
        gate.last_path.clone_from(&path_now);
        gate.ignore_est_left = FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE;
        gate.locked_disc_fps = None;
        gate.stable_streak = 0;
        gate.last_stable_fps = None;
        gate.interleaved_smooth = path_now.as_deref().is_some_and(mpv_path_is_disc);
    }
    if gate.ignore_est_left > 0 {
        gate.ignore_est_left -= 1;
        None
    } else {
        est
    }
}

pub(crate) fn video_log() -> bool {
    std::env::var("RHINO_VIDEO_LOG")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Stores a stable absolute path for SQLite ([crate::db::VideoPrefs::mvtools_lib]).
fn mvt_path_to_store(p: &std::path::Path) -> String {
    p.canonicalize()
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

/// Resolves the **MVTools** plugin file (`libmvtools.so` on Linux, `libmvtools.dylib` on macOS),
/// sets `RHINO_MVTOOLS_LIB` (in-process mpv inherits the environment).
/// Order: env [crate::paths::mvtools_from_env], then **cached** [crate::db::VideoPrefs::mvtools_lib] if still a file, else
/// [crate::paths::mvtools_lib_search]; on success, saves the full path in settings so the scan is not repeated
/// while the file exists. Returns `false` when MVTools cannot be resolved.
fn apply_mvtools_env(v: &mut crate::db::VideoPrefs) -> bool {
    if let Some(p) = crate::paths::mvtools_from_env() {
        let s = mvt_path_to_store(&p);
        if v.mvtools_lib != s {
            v.mvtools_lib = s;
            crate::db::save_video(v);
        }
        std::env::set_var(crate::paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
        eprintln!(
            "[rhino] video: libmvtools -> {} (from {})",
            v.mvtools_lib,
            crate::paths::RHINO_MVTOOLS_LIB_VAR
        );
        return true;
    }
    let c = v.mvtools_lib.trim();
    if !c.is_empty() {
        if std::path::Path::new(c).is_file() {
            std::env::set_var(crate::paths::RHINO_MVTOOLS_LIB_VAR, c);
            eprintln!("[rhino] video: libmvtools -> {c} (cached in settings)");
            return true;
        }
        v.mvtools_lib.clear();
        crate::db::save_video(v);
    }
    if let Some(p) = crate::paths::mvtools_lib_search() {
        v.mvtools_lib = mvt_path_to_store(&p);
        crate::db::save_video(v);
        std::env::set_var(crate::paths::RHINO_MVTOOLS_LIB_VAR, &v.mvtools_lib);
        eprintln!("[rhino] video: libmvtools -> {}", v.mvtools_lib);
        true
    } else {
        eprintln!(
            "[rhino] video: libmvtools not found; set {} or install MVTools (Linux: vsrepo / \
             distro package, macOS: `brew install mvtools`). See `data/vs/README.md`.",
            crate::paths::RHINO_MVTOOLS_LIB_VAR
        );
        false
    }
}

/// Preferred frame rate from mpv for Smooth cadence: `container-fps` with `estimated-vf-fps` tie-break.
///
/// Demux often reports ~24 Hz before `~24000/1001` film cadence stabilizes; when `estimated-vf-fps`
/// `estimated-vf-fps` may still describe the previous file — `mask_est_for_path_change` drops it briefly.
#[must_use]
pub(super) fn source_fps_from_mpv(mpv: &libmpv2::Mpv) -> Option<f64> {
    let cfps = mpv
        .get_property::<f64>("container-fps")
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0);
    let est = mpv
        .get_property::<f64>("estimated-vf-fps")
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0);
    let path_now = mpv
        .get_property::<String>("path")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let mut gate = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    let est = mask_est_for_path_change_with_state(path_now.clone(), est, &mut gate);
    let est = if ignore_est_for_source_pick(path_now.as_deref(), mpv) {
        None
    } else {
        est
    };
    let picked = source_fps_from_container_and_estimated(cfps, est);
    let picked = stabilize_disc_source_fps(path_now.as_deref(), picked, &mut gate);
    update_interleaved_cadence_gate(path_now.as_deref(), picked, &mut gate)
}

#[must_use]
fn source_fps_from_container_and_estimated(
    container: Option<f64>,
    estimated: Option<f64>,
) -> Option<f64> {
    const NTSC_FILM: f64 = 24000.0 / 1001.0;
    match (container, estimated) {
        (Some(c), Some(e)) => {
            if (c - 24.0).abs() < 0.02 && (e - NTSC_FILM).abs() < 0.09 {
                Some(e)
            } else {
                Some(c)
            }
        }
        (Some(c), None) => Some(c),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

#[cfg(test)]
mod source_fps_pick_tests {
    use super::mask_est_for_path_change_with_state;
    use super::mpv_path_is_disc;
    use super::source_fps_from_container_and_estimated;
    use super::stabilize_disc_source_fps;
    use super::update_interleaved_cadence_gate;
    use super::CADENCE_STABLE_READS;
    use super::FpsPickGateState;

    #[test]
    fn ntsc_film_prefers_estimated_when_container_rounds_to_24() {
        let ntsc = 24000.0 / 1001.0;
        assert!((source_fps_from_container_and_estimated(Some(24.0), Some(ntsc)).unwrap() - ntsc).abs() < 1e-6);
        assert!(
            (source_fps_from_container_and_estimated(Some(24.0), Some(24.0)).unwrap() - 24.0).abs() < 1e-6
        );
    }

    #[test]
    fn container_only_passthrough_including_24() {
        assert!((source_fps_from_container_and_estimated(Some(24.0), None).unwrap() - 24.0).abs() < 1e-6);
    }

    #[test]
    fn container_passthrough_when_no_estimate_non_24() {
        assert!(
            (source_fps_from_container_and_estimated(Some(29.97), None).unwrap() - 29.97).abs() < 1e-6
        );
    }

    #[test]
    fn container_passthrough_when_no_mismatch_with_estimate() {
        assert!(
            (source_fps_from_container_and_estimated(Some(29.97), Some(29.97)).unwrap() - 29.97).abs()
                < 1e-6
        );
    }

    #[test]
    fn fps_gate_skips_est_after_path_change_burst() {
        let mut g = FpsPickGateState::default();
        let ntsc = 24000.0 / 1001.0;
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g),
            Some(ntsc)
        );
    }

    #[test]
    fn interleaved_jump_keeps_display_resample_mode() {
        let mut g = FpsPickGateState::default();
        let path: Option<String> = Some("bd://1".into());
        let film = 24000.0 / 1001.0;
        let video = 30000.0 / 1001.0;
        for _ in 0..CADENCE_STABLE_READS {
            update_interleaved_cadence_gate(path.as_deref(), Some(film), &mut g);
        }
        assert!(!g.interleaved_smooth);
        update_interleaved_cadence_gate(path.as_deref(), Some(video), &mut g);
        assert!(g.interleaved_smooth);
    }

    #[test]
    fn disc_stabilizer_ignores_wild_est_after_plausible_container() {
        let mut g = FpsPickGateState::default();
        let path: Option<String> = Some("bd://1".into());
        let first = stabilize_disc_source_fps(path.as_deref(), Some(24000.0 / 1001.0), &mut g);
        assert!(first.is_some());
        assert_eq!(g.locked_disc_fps, first);
        assert_eq!(
            stabilize_disc_source_fps(path.as_deref(), Some(60.0), &mut g),
            first
        );
        assert_eq!(stabilize_disc_source_fps(path.as_deref(), Some(6.5), &mut g), first);
    }

    #[test]
    fn mpv_path_is_disc_helper() {
        assert!(mpv_path_is_disc("bd://foo"));
        assert!(mpv_path_is_disc("bluray://bar"));
        assert!(!mpv_path_is_disc("/movie.mkv"));
    }

    #[test]
    fn fps_gate_drops_stale_ntsc_after_opening_true_24_file() {
        let mut g = FpsPickGateState::default();
        let ntsc = 24000.0 / 1001.0;
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g),
            Some(ntsc)
        );
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(ntsc), &mut g),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(24.0), &mut g),
            Some(24.0)
        );
        assert!((source_fps_from_container_and_estimated(Some(24.0), Some(24.0)).unwrap() - 24.0).abs()
            < 1e-6);
    }
}

/// Publish [crate::paths::RHINO_SOURCE_FPS_VAR] for tools/legacy readers. The bundled `.vpy` prefers
/// [crate::paths::RHINO_SOURCE_FPS_VAR] for tools; the bundled `.vpy` uses libc **`getenv`** for this var (Linux and macOS).
pub(super) fn apply_source_fps_env(fps: Option<f64>) {
    match fps {
        Some(fps) => {
            std::env::set_var(crate::paths::RHINO_SOURCE_FPS_VAR, format!("{fps:.6}"));
            eprintln!(
                "[rhino] video: source fps -> {fps:.6} ({})",
                crate::paths::RHINO_SOURCE_FPS_VAR
            );
        }
        None => {
            std::env::remove_var(crate::paths::RHINO_SOURCE_FPS_VAR);
            eprintln!(
                "[rhino] video: source fps unknown (mpv has no `container-fps` / `estimated-vf-fps`) — script will passthrough"
            );
        }
    }
}
