// RHINO_VIDEO_LOG, libmvtools resolution into RHINO_MVTOOLS_LIB, and RHINO_SOURCE_FPS from mpv.

use std::sync::Mutex;

pub(crate) fn mpv_path_is_disc(path: &str) -> bool {
    let p = path.trim().to_ascii_lowercase();
    p.starts_with("bd://")
        || p.starts_with("bluray://")
        || p.starts_with("dvd://")
}

fn path_str_is_dvd_vob(path: Option<&str>) -> bool {
    path.and_then(crate::media_probe::local_path_from_mpv_str)
        .is_some_and(|p| crate::video_ext::is_dvd_vob_path(&p))
}

fn shell_path_is_dvd_vob(shell: Option<&std::path::Path>) -> bool {
    shell.is_some_and(crate::video_ext::is_dvd_vob_path)
}

fn media_is_dvd_vob(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> bool {
    crate::video_pref::me_budget_local_path(mpv, bundle)
        .is_some_and(|p| crate::video_ext::is_dvd_vob_path(&p))
}

/// True when mpv's `vf` chain includes a **vapoursynth** filter.
pub(crate) fn vf_chain_has_vapoursynth(mpv: &libmpv2::Mpv) -> bool {
    mpv.get_property::<String>("vf")
        .map(|s| s.to_ascii_lowercase().contains("vapoursynth"))
        .unwrap_or(false)
}

include!("mvtools_cadence_gate.rs");

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
pub(super) fn source_fps_from_mpv(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> Option<f64> {
    let shell_media = crate::video_pref::me_budget_local_path(mpv, bundle);
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
    let est = mask_est_for_path_change_with_state(
        path_now.clone(),
        est,
        &mut gate,
        shell_media.as_deref(),
    );
    let est = if ignore_est_for_source_pick(path_now.as_deref(), mpv, shell_media.as_deref()) {
        None
    } else {
        est
    };
    let mut picked = source_fps_from_container_and_estimated(cfps, est);
    picked = stabilize_disc_source_fps(path_now.as_deref(), picked, &mut gate);
    if media_is_dvd_vob(mpv, bundle) {
        let fps =
            crate::video_ext::dvd_vob_broadcast_fps(crate::video_pref::decode_wh_from_mpv(mpv))
                .or(Some(25.0));
        picked = fps;
        gate.interleaved_smooth = false;
        gate.stable_streak = CADENCE_STABLE_READS;
        gate.last_stable_fps = fps;
    }
    update_interleaved_cadence_gate(path_now.as_deref(), picked, &mut gate)
}

/// Gate-only cadence pick for [super::apply_mpv_video]; publish env via [apply_source_fps_env] once.
pub(super) fn refresh_smooth_cadence_gate(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> Option<f64> {
    source_fps_from_mpv(mpv, bundle)
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
                mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g, None),
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
        assert!(mpv_path_is_disc("dvd://1"));
        assert!(!mpv_path_is_disc("/movie.mkv"));
    }

    #[test]
    fn fps_gate_drops_stale_ntsc_after_opening_true_24_file() {
        let mut g = FpsPickGateState::default();
        let ntsc = 24000.0 / 1001.0;
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g, None),
            Some(ntsc)
        );
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(24.0), &mut g, None),
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
            let s = format!("{fps:.6}");
            if std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR).ok().as_deref() == Some(s.as_str()) {
                return;
            }
            std::env::set_var(crate::paths::RHINO_SOURCE_FPS_VAR, &s);
            eprintln!(
                "[rhino] video: source fps -> {fps:.6} ({})",
                crate::paths::RHINO_SOURCE_FPS_VAR
            );
        }
        None => {
            if std::env::var_os(crate::paths::RHINO_SOURCE_FPS_VAR).is_none() {
                eprintln!(
                    "[rhino] video: source fps unknown (mpv has no `container-fps` / `estimated-vf-fps`) — script will passthrough"
                );
                return;
            }
            std::env::remove_var(crate::paths::RHINO_SOURCE_FPS_VAR);
            eprintln!(
                "[rhino] video: source fps unknown (mpv has no `container-fps` / `estimated-vf-fps`) — script will passthrough"
            );
        }
    }
}
