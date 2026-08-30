//! XDG config: `~/.config/rhino/…` and project data paths (bundled [`.vpy`] for VapourSynth).
//! [mvtools_from_env] / [mvtools_lib_search] find the **MVTools** plugin file
//! (`libmvtools.so` on Linux; `mvtools.dylib` / legacy `libmvtools.dylib` on macOS). The app caches the path in SQLite and sets
//! `RHINO_MVTOOLS_LIB` (see `video_pref`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// `~/.config/rhino` (created if possible). `None` if `HOME` / config base is missing.
pub fn app_config() -> Option<PathBuf> {
    let dir = xdg_config_base()?.join("rhino");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// `XDG_CONFIG_HOME` when absolute, else `$HOME/.config`.
fn xdg_config_base() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

const BUNDLED_MVT60_VPY: &str = "rhino_60_mvtools.vpy";

fn macos_app_contents_from_exe(bin_dir: &Path) -> Option<&Path> {
    if bin_dir.file_name() != Some(OsStr::new("MacOS")) {
        return None;
    }
    bin_dir.parent()
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if !out.iter().any(|e| e == &p) {
            out.push(p);
        }
    }
    out
}

/// Roots that may contain `share/rhino-player/vs` next to `current_exe`:
/// **`PREFIX/share`** when the binary is **`PREFIX/bin/…`**; **`Contents/Resources`** and **`Contents`**
/// when it is **`…/Contents/MacOS/…`** (macOS `.app`).
fn share_roots_next_to_exe() -> Vec<PathBuf> {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let Some(bin_dir) = exe.parent() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(contents) = macos_app_contents_from_exe(bin_dir) {
        let res = contents.join("Resources");
        if res.is_dir() {
            out.push(res);
        }
        out.push(contents.to_path_buf());
    }
    if let Some(prefix) = bin_dir.parent() {
        out.push(prefix.to_path_buf());
    }
    dedupe_paths(out)
}

/// **Freedesktop hicolor tree** bundled inside a shipped macOS `.app` (`Contents/Resources/data/icons`).
pub fn bundled_data_icons_dir_for_running_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let contents = macos_app_contents_from_exe(bin_dir)?;
    let icons = contents.join("Resources/data/icons");
    icons.is_dir().then_some(icons)
}

/// Prefers **`.app` Resources**, then **`PREFIX/share/rhino-player/icons`** (Homebrew / `.deb`
/// layout next to `bin/`), then the compile-time **`data/icons`** checkout.
pub fn bundled_data_icons_dir_for_runtime() -> Option<PathBuf> {
    bundled_data_icons_dir_for_running_exe()
        .or_else(icons_dir_next_to_exe)
        .or_else(|| {
            let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/icons");
            p.is_dir().then_some(p)
        })
}

/// `PREFIX/share/rhino-player/icons` when the binary lives in **`PREFIX/bin`** (Homebrew, `.deb`).
fn icons_dir_next_to_exe() -> Option<PathBuf> {
    for base in share_roots_next_to_exe() {
        let p = base.join("share/rhino-player/icons");
        if p.is_dir() {
            return std::fs::canonicalize(&p).ok().or(Some(p));
        }
    }
    None
}

/// Bundled `data/vs/…/rhino_60_mvtools.vpy` when **Preferences** → VapourSynth is active and DB
/// `video_vs_path` is empty.
pub fn bundled_mvtools_60() -> Option<PathBuf> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data/vs")
        .join(BUNDLED_MVT60_VPY);
    if dev.is_file() {
        return Some(dev);
    }
    for base in share_roots_next_to_exe() {
        let p = base.join("share/rhino-player/vs").join(BUNDLED_MVT60_VPY);
        if p.is_file() {
            return std::fs::canonicalize(&p).ok().or(Some(p));
        }
    }
    None
}

/// Environment key for the absolute path to the **MVTools** plugin file (Rhino and bundled
/// `.vpy` `LoadPlugin`). Basename: `libmvtools.so` (Linux) or `mvtools.dylib` (macOS).
pub const RHINO_MVTOOLS_LIB_VAR: &str = "RHINO_MVTOOLS_LIB";

// Bundled ME px²: `RHINO_SMOOTH_MAX_AREA` (see `paths_smooth_me_budget_env`).

include!("paths_smooth_me_budget_env.rs");
include!("paths_mvtools_macos.rs");
include!("paths_mvtools_linux.rs");
include!("paths_vapoursynth_macos.rs");

/// Playback speed (e.g. `1.0`, `1.5`, `2.0`, `8.0`) for the bundled `rhino_60_mvtools.vpy` so **FlowFPS** only fills
/// frames to **~60** against **(source fps × speed)**. Set with [crate::video_pref::set_playback_speed_env_from_mpv] or [crate::video_pref::set_playback_speed_env] (known UI value) before the vf is built.
pub const RHINO_PLAYBACK_SPEED_VAR: &str = "RHINO_PLAYBACK_SPEED";

/// Source frames-per-second (decimal, e.g. `29.970029970`) Rhino sets from mpv's `container-fps` / **`estimated-vf-fps`**
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

/// **`RHINO_SMOOTH_DROP_STATS=1`** stderr **≈5 s** strain tallies (**mistimed** → VO **`frame-drop-count`** → decoder) while Smooth bundled **`vf`** is active (see **`smooth_budget`**).
pub const RHINO_SMOOTH_DROP_STATS_VAR: &str = "RHINO_SMOOTH_DROP_STATS";

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
///   broader walk of `~/.local` ([paths_mvtools_linux]).
/// - **macOS**: `.app` **`Contents/Resources/lib/vapoursynth/plugins/mvtools.dylib`**, then
///   **`~/.config/rhino/lib/…`** (seeded once), then Homebrew **`vapoursynth-mvtools`** /
///   legacy **`libmvtools.dylib`**. vsrepo is Linux-only.
pub fn mvtools_lib_search() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        macos_mvtools_lib_search()
    }
    #[cfg(not(target_os = "macos"))]
    {
        linux_mvtools_lib_search()
    }
}
