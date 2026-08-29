// Media duration helpers for the continue grid / transport.
// Submodule of history (see `media_duration` in [history_and_media_playback.rs]).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, OptionalExtension};

use super::{history_key, with_conn};

pub fn load_duration_map() -> HashMap<String, f64> {
    with_conn(|c| {
        let mut s = c.prepare(
            "SELECT path, duration_sec FROM media WHERE duration_sec IS NOT NULL AND duration_sec > 0",
        )?;
        let m = s.query_map([], |row| {
            let p: String = row.get(0)?;
            let d: f64 = row.get(1)?;
            Ok((p, d))
        })?;
        Ok(m.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

/// Stored length (seconds) for one path — browse seek-bar hover when the strip cache misses.
pub fn media_duration_sec(path: &Path) -> Option<f64> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT duration_sec FROM media WHERE path = ?1",
            params![&s],
            |row| row.get::<_, Option<f64>>(0),
        )
        .optional()
    })
    .flatten()
    .flatten()
    .filter(|d| d.is_finite() && *d > 0.0)
}

pub fn set_duration(path: &Path, sec: f64) {
    if !sec.is_finite() || sec <= 0.0 {
        return;
    }
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, duration_sec) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET duration_sec = excluded.duration_sec",
            params![&s, sec],
        )?;
        Ok(())
    });
}

/// Drop a stale whole-title duration while keeping resume (legacy whole-disc rows).
pub fn clear_duration(path: &Path) {
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "UPDATE media SET duration_sec = NULL WHERE path = ?1",
            params![&s],
        )?;
        Ok(())
    });
}
