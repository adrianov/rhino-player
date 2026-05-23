//! Recent file paths, stored in the central DB ([crate::db]). See `docs/features/21-recent-videos-launch.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX: usize = 20;

/// Ordered recent paths (newest first), up to [MAX] entries. Entries whose paths **no longer exist**
/// on disk are dropped from history and resume, then **omitted** from the result.
pub fn load() -> Vec<PathBuf> {
    let raw = crate::db::list_history(MAX);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in raw {
        if !p.exists() {
            crate::media_probe::remove_continue_entry(&p);
            continue;
        }
        let entity = crate::playback_entity::db_path_for(&p);
        let Some(entity_key) = crate::db::history_key(&entity) else {
            continue;
        };
        if !seen.insert(entity_key.clone()) {
            crate::db::delete_history_stored_path(&p);
            continue;
        }
        out.push(entity);
    }
    out
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
    let ent = crate::playback_entity::PlaybackEntity::resolve(path);
    ent.purge_extra_db_rows();
}
