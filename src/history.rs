//! Recent file paths, stored in the central DB ([crate::db]). See `docs/features/21-recent-videos-launch.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX: usize = 20;

/// Ordered recent paths (newest first), up to [MAX] entries. Missing paths leave history
/// and the files catalog — unless an incomplete download was renamed to a finished sibling, in which
/// case the persistent store adopts that finished path and the entry is kept.
pub fn load() -> Vec<PathBuf> {
    let raw = crate::db::list_history(MAX);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in raw {
        let Some(p) = adopt_finished_download(p) else {
            continue;
        };
        let entity = crate::playback_entity::db_path_for(&p);
        let Some(entity_key) = crate::db::history_key(&entity) else {
            continue;
        };
        if !seen.insert(entity_key.clone()) {
            // Drop redundant stored keys (e.g. extra DVD chapter rows), never the kept entity key.
            if p.to_str() != Some(entity_key.as_str()) {
                crate::db::delete_history_stored_path(&p);
            }
            continue;
        }
        out.push(entity);
    }
    out
}

/// Keep existing paths; adopt a finished sibling for a gone incomplete download; else prune.
fn adopt_finished_download(p: PathBuf) -> Option<PathBuf> {
    if p.exists() {
        return Some(p);
    }
    crate::media_probe::forget_missing(&p)
}

/// Insert at front, dedupe, trim; one row per DVD title (not per chapter `.vob`).
pub fn record(path: &Path) {
    let key = crate::playback_entity::db_path_for(path);
    crate::db::remove_history_matching_entity(&key);
    crate::db::record_history(&key);
}

/// Remove one path (DVD titles: entity key + legacy folder/chapter rows in SQLite).
pub fn remove(path: &Path) {
    crate::db::remove_history_matching_entity(path);
    crate::playback_entity::PlaybackEntity::resolve(path).purge_extra_db_rows();
}
