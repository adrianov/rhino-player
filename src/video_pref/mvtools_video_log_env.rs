// RHINO_VIDEO_LOG, libmvtools resolution into RHINO_MVTOOLS_LIB, and RHINO_SOURCE_FPS from mpv.

use std::sync::Mutex;

/// After `loadfile`, `estimated-vf-fps` can still reflect the previous clip longer than one idle tick.
/// Pairing that stale value with the new file’s `container-fps` (often ~24) incorrectly triggers the
/// NTSC film tie-break. Drop `estimated-vf-fps` for the first `FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE`
/// `source_fps_from_mpv` reads after `path` changes (several rebuilds / resyncs can run before mpv updates).
const FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE: u32 = 6;
#[derive(Debug, Clone, Default)]
struct FpsPickGateState {
    last_path: Option<String>,
    ignore_est_left: u32,
}

static FPS_PICK_GATE: Mutex<FpsPickGateState> = Mutex::new(FpsPickGateState {
    last_path: None,
    ignore_est_left: 0,
});

fn mask_est_for_path_change_with_state(
    path_now: Option<String>,
    est: Option<f64>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let path_changed = gate.last_path != path_now;
    if path_changed {
        gate.last_path.clone_from(&path_now);
        gate.ignore_est_left = FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE;
    }
    if gate.ignore_est_left > 0 {
        gate.ignore_est_left -= 1;
        None
    } else {
        est
    }
}

fn mask_est_for_path_change(path_now: Option<String>, est: Option<f64>) -> Option<f64> {
    let mut g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    mask_est_for_path_change_with_state(path_now, est, &mut g)
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
    let est = mask_est_for_path_change(path_now, est);
    source_fps_from_container_and_estimated(cfps, est)
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
    use super::source_fps_from_container_and_estimated;
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
