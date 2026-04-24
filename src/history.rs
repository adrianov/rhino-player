//! Recent file paths, stored in the central DB ([crate::db]). See `docs/features/21-recent-videos-launch.md`.

use std::path::{Path, PathBuf};

const MAX: usize = 20;

/// Ordered recent paths (newest first), up to [MAX] entries.
pub fn load() -> Vec<PathBuf> {
    crate::db::list_history(MAX)
}

/// Insert at front, dedupe, trim; canonical path.
pub fn record(path: &Path) {
    crate::db::record_history(path);
}

/// Remove one path.
pub fn remove(path: &Path) {
    crate::db::remove_history(path);
}
