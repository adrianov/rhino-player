//! XDG config: `~/.config/rhino/…`.

use std::path::PathBuf;

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
