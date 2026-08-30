// Before GTK / libadwaita init: make Homebrew GSettings schemas discoverable when the
// process was started outside a login shell (Finder / Dock / `open` on a `.app`).
// Without this, libadwaita aborts with "No GSettings schemas are installed on the system".

/// Prepend `dir` to a `:`-separated env var when it is not already a path entry.
#[cfg(target_os = "macos")]
fn prepend_path_env(key: &str, dir: &Path) {
    let dir_s = dir.to_string_lossy();
    let merged = match std::env::var_os(key) {
        Some(cur) if !cur.is_empty() => {
            let cur_s = cur.to_string_lossy();
            if cur_s.split(':').any(|p| p == dir_s) {
                return;
            }
            format!("{dir_s}:{cur_s}")
        }
        _ => dir_s.into_owned(),
    };
    // SAFETY: single-threaded startup before GTK; no other threads read env yet.
    unsafe {
        std::env::set_var(key, merged);
    }
}

/// First Homebrew prefix whose `share/glib-2.0/schemas` tree exists.
#[cfg(target_os = "macos")]
fn homebrew_share_with_schemas() -> Option<PathBuf> {
    ["/opt/homebrew", "/usr/local"].iter().find_map(|prefix| {
        let share = Path::new(prefix).join("share");
        share
            .join("glib-2.0/schemas")
            .is_dir()
            .then_some(share)
    })
}

/// Ensure Homebrew's `share` is on `XDG_DATA_DIRS` so GLib finds compiled schemas.
/// Safe to call more than once; no-ops when Homebrew GTK is absent.
#[cfg(target_os = "macos")]
pub fn macos_prime_homebrew_runtime_env() {
    let Some(share) = homebrew_share_with_schemas() else {
        return;
    };
    prepend_path_env("XDG_DATA_DIRS", &share);
}
