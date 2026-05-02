// RHINO_VIDEO_LOG, libmvtools resolution into RHINO_MVTOOLS_LIB, and RHINO_SOURCE_FPS from mpv.

fn video_log() -> bool {
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

/// Publish [crate::paths::RHINO_SOURCE_FPS_VAR] so the bundled `.vpy` can recover a real source cadence
/// when mpv's vapoursynth filter passes `fps_num=0 / fps_den=0` to the script (it does this for
/// many otherwise-CFR mp4s — phone captures, screen recordings, web exports). Reads `container-fps`
/// (mpv's container-reported rate); on miss tries `estimated-vf-fps` as a last-ditch sample, then
/// clears the env so the script's safe passthrough kicks in instead of a stale value from a
/// previous file.
fn set_source_fps_env_from_mpv(mpv: &libmpv2::Mpv) {
    let cfps = mpv
        .get_property::<f64>("container-fps")
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0);
    let est = || {
        mpv.get_property::<f64>("estimated-vf-fps")
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    match cfps.or_else(est) {
        Some(fps) => {
            std::env::set_var(crate::paths::RHINO_SOURCE_FPS_VAR, format!("{fps:.6}"));
            eprintln!(
                "[rhino] video: source fps -> {fps:.6} ({})",
                crate::paths::RHINO_SOURCE_FPS_VAR
            );
        }
        None => {
            std::env::remove_var(crate::paths::RHINO_SOURCE_FPS_VAR);
            eprintln!("[rhino] video: source fps unknown (mpv has no `container-fps`) — script will passthrough");
        }
    }
}
