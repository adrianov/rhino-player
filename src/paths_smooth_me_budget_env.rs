// Snapfile (`RHINO_SMOOTH_CAP_FILE`) — authoritative bundled ME px² for Rhino before `vf add`.
// Standalone `mpv` + bundled script may use `RHINO_SMOOTH_MAX_AREA` when this path is unset.

/// Env key: absolute path to a single-line ME px² text file (Rhino sets before **`vf add`**).
pub const RHINO_SMOOTH_CAP_FILE_VAR: &str = "RHINO_SMOOTH_CAP_FILE";

/// `XDG_CACHE_HOME/rhino/smooth-me-cap-<pid>.txt` (else `~/.cache/…`). **`None`** if no HOME/cache base.
#[must_use]
pub fn smooth_me_cap_snap_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME").and_then(|v| {
        let p = PathBuf::from(v);
        p.is_absolute().then_some(p)
    });
    let base = base.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = base.join("rhino");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("smooth-me-cap-{}.txt", std::process::id())))
}

/// Writes **`cap_px`** to [smooth_me_cap_snap_path] and exports [RHINO_SMOOTH_CAP_FILE_VAR]. Failures are ignored.
pub fn publish_smooth_me_cap_snap(cap_px: u64) {
    let Some(path) = smooth_me_cap_snap_path() else {
        return;
    };
    if std::fs::write(&path, format!("{cap_px}\n")).is_err() {
        return;
    }
    std::env::set_var(RHINO_SMOOTH_CAP_FILE_VAR, path.as_os_str());
}

/// **`true`** when [RHINO_SMOOTH_CAP_FILE_VAR] names an existing file whose first line parses to **`want_px`**.
#[must_use]
pub fn smooth_me_cap_snap_content_equals(want_px: u64) -> bool {
    let Ok(path) = std::env::var(RHINO_SMOOTH_CAP_FILE_VAR) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines()
        .next()
        .map(str::trim)
        .and_then(|ln| ln.parse::<u64>().ok())
        == Some(want_px)
}
