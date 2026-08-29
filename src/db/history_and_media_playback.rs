use std::path::{Path, PathBuf};

// Split-out unit (resume position + playback + track storage); public paths stay stable.
mod playback_state {
    include!("history_playback_state.rs");
}
pub use playback_state::*;

// Path catalog (`files` table) — feature 34 partial; neighbour search seeds from here.
#[path = "history_files_catalog.rs"]
mod files_catalog;
pub use files_catalog::*;

#[path = "history_media_duration.rs"]
mod media_duration;
pub use media_duration::*;

/// Newest first, at most [MAX_HISTORY] kept.
pub fn list_history(limit: usize) -> Vec<PathBuf> {
    with_conn(|c| {
        let lim = (limit as i64).min(MAX_HISTORY);
        let mut s = c.prepare("SELECT path FROM history ORDER BY last_opened DESC LIMIT ?1")?;
        let it = s.query_map([lim], |row| {
            let p: String = row.get(0)?;
            Ok(PathBuf::from(p))
        })?;
        Ok(it.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

pub fn record_history(path: &Path) {
    let Some(s) = history_key(path) else {
        return;
    };
    let now = now_unix_ms();
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO history (path, last_opened) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET
               last_opened = MAX(history.last_opened, excluded.last_opened)",
            params![&s, now],
        )?;
        c.execute(
            "DELETE FROM history WHERE id NOT IN (
                 SELECT id FROM (
                     SELECT id FROM history ORDER BY last_opened DESC LIMIT ?1
                 )
             )",
            params![MAX_HISTORY],
        )?;
        Ok(())
    });
    // Continue / open always registers the path in the files catalog (feature 34).
    ensure_file(Path::new(&s));
}

pub(crate) fn history_key(path: &Path) -> Option<String> {
    let key = crate::playback_entity::db_path_for(path);
    std::fs::canonicalize(&key)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .or_else(|| key.to_str().map(str::to_string))
}

/// SQLite `media.path` for one filesystem object — no playback-entity remap.
pub(crate) fn media_path_key_exact(path: &Path) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .or_else(|| path.to_str().map(str::to_owned))
}

/// Remove `media` row for this exact path string (no DVD entity remap).
pub fn delete_media_row_exact(path: &Path) {
    let Some(s) = media_path_key_exact(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute("DELETE FROM media WHERE path = ?1", params![&s])?;
        Ok(())
    });
}

pub fn remove_history(path: &Path) {
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute("DELETE FROM history WHERE path = ?1", params![&s])?;
        Ok(())
    });
}

/// Delete one `history` row by the exact path string stored in SQLite (not remapped).
pub fn delete_history_stored_path(path: &Path) {
    let Some(s) = path.to_str() else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute("DELETE FROM history WHERE path = ?1", params![s])?;
        Ok(())
    });
}

/// Drop every continue-list row for the same [crate::playback_entity] (folder, chapter, entity key, …).
pub fn remove_history_matching_entity(path: &Path) {
    let Some(target) = history_key(&crate::playback_entity::db_path_for(path)) else {
        return;
    };
    for p in list_history(MAX_HISTORY as usize) {
        if history_key(&crate::playback_entity::db_path_for(&p)).as_deref() == Some(target.as_str())
        {
            delete_history_stored_path(&p);
        }
    }
    remove_history(&crate::playback_entity::db_path_for(path));
}
