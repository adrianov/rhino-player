//! XDG config: `~/.config/rhino/…` and project data paths (bundled [`.vpy`] for VapourSynth).
//! [mvtools_from_env] / [mvtools_lib_search] find the **MVTools** plugin file
//! (`libmvtools.so` on Linux, `libmvtools.dylib` on macOS); the app caches the path in SQLite and
//! sets the `RHINO_MVTOOLS_LIB` env (see `video_pref` `apply_mvtools_env`).

use std::path::{Path, PathBuf};

/// `~/.config/rhino` (created if possible). `None` if `HOME` / config base is missing.
pub fn app_config() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").and_then(|v| {
        let p = PathBuf::from(v);
        p.is_absolute().then_some(p)
    });
    let base =
        base.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    let dir = base.join("rhino");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

const BUNDLED_MVT60_VPY: &str = "rhino_60_mvtools.vpy";

/// Bundled `data/vs/…/rhino_60_mvtools.vpy` when **Preferences** → VapourSynth is active and DB
/// `video_vs_path` is empty.
pub fn bundled_mvtools_60() -> Option<PathBuf> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data/vs")
        .join(BUNDLED_MVT60_VPY);
    if dev.is_file() {
        return Some(dev);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let c = dir.join("../share/rhino-player/vs").join(BUNDLED_MVT60_VPY);
            if c.is_file() {
                return std::fs::canonicalize(&c).ok().or(Some(c));
            }
        }
    }
    None
}

/// Plugin file basename for the Linux pipx/vsrepo fallback search. Linux ships MVTools as
/// `libmvtools.so`. macOS uses fixed Homebrew paths in [DISTRO_MVTOOLS_PATHS] (where the file
/// is `libmvtools.dylib`) and never falls back to a name-based scan.
#[cfg(not(target_os = "macos"))]
const MVTOOLS_FILE: &str = "libmvtools.so";

/// Environment key for the absolute path to the **MVTools** plugin file (Rhino and bundled
/// `.vpy` `LoadPlugin`). The basename differs per OS — see [MVTOOLS_FILE].
pub const RHINO_MVTOOLS_LIB_VAR: &str = "RHINO_MVTOOLS_LIB";

/// Playback speed (e.g. `1.0`, `1.5`, `2.0`, `8.0`) for the bundled `rhino_60_mvtools.vpy` so **FlowFPS** only fills
/// frames to **~60** against **(source fps × speed)**. Set with [crate::video_pref::set_playback_speed_env_from_mpv] or [crate::video_pref::set_playback_speed_env] (known UI value) before the vf is built.
pub const RHINO_PLAYBACK_SPEED_VAR: &str = "RHINO_PLAYBACK_SPEED";

/// Source frames-per-second (decimal, e.g. `29.970029970`) Rhino sets from mpv's `container-fps`
/// before attaching the bundled `rhino_60_mvtools.vpy`. mpv's vapoursynth filter often passes
/// `fps_num=0 / fps_den=0` to the script even when the container is CFR (29.970, 23.976, etc.);
/// the script falls back to this value and rationalizes it (e.g. `30000/1001`) so FlowFPS gets
/// a real cadence instead of the old hardcoded `24000/1001` which silently stretched 29.97
/// content by 25 %.
pub const RHINO_SOURCE_FPS_VAR: &str = "RHINO_SOURCE_FPS";

/// Bumped in-process before each `vf add vapoursynth` so the bundled `.vpy` can stderr-log **once**
/// per interpreter for that attach when `RHINO_VPY_LOG_EPOCH` is set (mpv may still re-run the script in a
/// new interpreter after seek).
pub const RHINO_VPY_LOG_EPOCH_VAR: &str = "RHINO_VPY_LOG_EPOCH";

/// [RHINO_MVTOOLS_LIB_VAR] if set to an existing file; otherwise `None`.
pub fn mvtools_from_env() -> Option<PathBuf> {
    let p = std::env::var(RHINO_MVTOOLS_LIB_VAR).ok()?;
    let b = PathBuf::from(p.trim());
    b.is_file()
        .then(|| std::fs::canonicalize(&b).ok().unwrap_or(b))
}

/// **Search only** (no env, no SQLite cache).
///
/// - **Linux**: common distro paths, **pipx vsrepo** under `~/.local/share/pipx/venvs/…`, then a
///   broader walk of `~/.local` (see [find_file_breadth_first]).
/// - **macOS**: Homebrew prefix `lib/` (`/opt/homebrew/lib` Apple Silicon, `/usr/local/lib` Intel)
///   where `brew install mvtools` drops `libmvtools.dylib`; vsrepo is Linux-only and there is no
///   pipx layout to scan.
pub fn mvtools_lib_search() -> Option<PathBuf> {
    for c in DISTRO_MVTOOLS_PATHS {
        let p = Path::new(c);
        if p.is_file() {
            return std::fs::canonicalize(p).ok().or(Some(p.to_path_buf()));
        }
    }
    extra_mvtools_search()
}

#[cfg(target_os = "macos")]
fn extra_mvtools_search() -> Option<PathBuf> {
    None
}

#[cfg(not(target_os = "macos"))]
fn extra_mvtools_search() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let local = PathBuf::from(home).join(".local");
    mvtools_in_pipx_venvs(&local)
        .or_else(|| find_file_breadth_first(&local, MVTOOLS_FILE, 14, 8000))
}

#[cfg(target_os = "macos")]
const DISTRO_MVTOOLS_PATHS: &[&str] = &[
    "/opt/homebrew/lib/libmvtools.dylib",
    "/usr/local/lib/libmvtools.dylib",
];

#[cfg(not(target_os = "macos"))]
const DISTRO_MVTOOLS_PATHS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/vapoursynth/libmvtools.so",
    "/usr/lib/vapoursynth/libmvtools.so",
    "/usr/local/lib/vapoursynth/libmvtools.so",
];

/// `~/.local/share/pipx/venvs/<name>/lib/python*/site-packages/vapoursynth/plugins/vsrepo/libmvtools.so`
/// (e.g. vsrepo in a pipx venv) — checked before scanning all of `~/.local`. Linux-only:
/// `vsrepo` is not used in the macOS Homebrew layout.
#[cfg(not(target_os = "macos"))]
fn mvtools_in_pipx_venvs(local: &Path) -> Option<PathBuf> {
    let venvs_root = local.join("share/pipx/venvs");
    let venvs = std::fs::read_dir(&venvs_root).ok()?;
    for venv in venvs.flatten() {
        let Ok(vft) = venv.file_type() else {
            continue;
        };
        if !vft.is_dir() {
            continue;
        }
        let lib = venv.path().join("lib");
        let pys = std::fs::read_dir(&lib).ok()?;
        for py in pys.flatten() {
            let Ok(pft) = py.file_type() else {
                continue;
            };
            if !pft.is_dir() {
                continue;
            }
            if !py.file_name().to_string_lossy().starts_with("python") {
                continue;
            }
            let p = py
                .path()
                .join("site-packages/vapoursynth/plugins/vsrepo")
                .join(MVTOOLS_FILE);
            if p.is_file() {
                return std::fs::canonicalize(&p).ok().or(Some(p));
            }
        }
    }
    None
}

/// Breadth-first search for `file_name` under `root`, at most `max_depth` directory levels from
/// `root`, stopping after `max_dir_reads` `read_dir` calls (avoids huge trees). Symlink directories
/// are not descended (same idea as Python `follow_symlinks=False`), so cycles do not burn the read budget.
/// Used only by the Linux pipx/vsrepo fallback; macOS uses Homebrew's fixed prefix instead.
#[cfg(not(target_os = "macos"))]
fn find_file_breadth_first(
    root: &Path,
    file_name: &str,
    max_depth: u32,
    max_dir_reads: usize,
) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    use std::collections::VecDeque;
    let mut q: VecDeque<(PathBuf, u32)> = VecDeque::new();
    q.push_back((root.to_path_buf(), 0));
    let mut reads = 0usize;
    while let Some((dir, depth)) = q.pop_front() {
        if reads >= max_dir_reads {
            return None;
        }
        reads += 1;
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in read.flatten() {
            let Ok(ft) = e.file_type() else {
                continue;
            };
            let p = e.path();
            // Do not follow symlink directories (avoids cycles and matches Python's follow_symlinks=False).
            if ft.is_dir() && !ft.is_symlink() {
                if depth < max_depth {
                    q.push_back((p, depth + 1));
                }
            } else if p.file_name().is_some_and(|f| f == file_name) {
                return std::fs::canonicalize(&p).ok().or(Some(p));
            }
        }
    }
    None
}
