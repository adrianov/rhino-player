//! Recent file paths, stored in the central DB ([crate::db]). See `docs/features/21-recent-videos-launch.md`.

use std::path::{Path, PathBuf};

const MAX: usize = 20;

/// Ordered recent paths (newest first), up to [MAX] entries. Entries whose paths **no longer exist**
/// on disk are dropped from history and resume, then **omitted** from the result.
pub fn load() -> Vec<PathBuf> {
    let raw = crate::db::list_history(MAX);
    let mut out = Vec::new();
    for p in raw {
        if p.exists() {
            out.push(p);
        } else {
            crate::media_probe::remove_continue_entry(&p);
        }
    }
    out
}

/// Insert at front, dedupe, trim; canonical path.
pub fn record(path: &Path) {
    crate::db::record_history(path);
}

/// Remove one path.
pub fn remove(path: &Path) {
    crate::db::remove_history(path);
}
