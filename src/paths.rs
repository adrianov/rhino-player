//! XDG config: `~/.config/rhino/…` and project data paths (bundled [`.vpy`] for VapourSynth).

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

/// Per-file resume data for mpv (`--watch-later-dir`), isolated from the standalone `mpv` CLI.
pub fn watch_later() -> Option<PathBuf> {
    let d = app_config()?.join("watch_later");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Bundled `data/vs/…` when **Preferences** → VapourSynth is active and DB `video_vs_path` is empty.
/// Prefers `rhino_60_mvtools_multicore.vpy`, else `rhino_60_mvtools.vpy` (fast preset).
pub fn bundled_mvtools_60() -> Option<PathBuf> {
    for name in [
        "rhino_60_mvtools_multicore.vpy",
        "rhino_60_mvtools.vpy",
    ] {
        let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vs").join(name);
        if dev.is_file() {
            return Some(dev);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let c = dir.join(format!("../share/rhino-player/vs/{name}"));
                if c.is_file() {
                    return std::fs::canonicalize(&c).ok().or(Some(c));
                }
            }
        }
    }
    None
}
