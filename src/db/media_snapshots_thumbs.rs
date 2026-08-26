// Split-out unit (thumbnail reuse/store rules); public paths stay stable.
mod thumb_cache {
    include!("media_snapshots_thumbs_cache.rs");
}
pub use thumb_cache::*;

/// Full `media` row for undo after “remove from list”; [path_key] is the same as [history_key] strings.
#[derive(Debug, Clone)]
pub struct MediaRowSnapshot {
    pub path_key: String,
    pub duration_sec: Option<f64>,
    pub time_pos_sec: Option<f64>,
    pub source_mtime_sec: Option<i64>,
    pub thumb_webp: Option<Vec<u8>>,
    pub thumb_time_pos_sec: Option<f64>,
    pub audio_aid: Option<i64>,
}

/// Read the row for this path, if any.
pub fn snapshot_media_row(path: &Path) -> Option<MediaRowSnapshot> {
    let path_key = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT path, duration_sec, time_pos_sec, source_mtime_sec, thumb_webp, thumb_time_pos_sec, audio_aid
             FROM media WHERE path = ?1",
            params![&path_key],
            snapshot_from_row,
        )
        .optional()
    })
    .flatten()
}

fn snapshot_from_row(row: &rusqlite::Row) -> rusqlite::Result<MediaRowSnapshot> {
    Ok(MediaRowSnapshot {
        path_key: row.get(0)?,
        duration_sec: row.get(1)?,
        time_pos_sec: row.get(2)?,
        source_mtime_sec: row.get(3)?,
        thumb_webp: row.get(4)?,
        thumb_time_pos_sec: row.get(5)?,
        audio_aid: row.get(6)?,
    })
}

/// WebP bytes for this entity path key when present (no mtime check).
pub fn stored_thumb_webp(path: &Path) -> Option<Vec<u8>> {
    let s = history_key(path)?;
    let b = with_conn(|c| {
        c.query_row(
            "SELECT thumb_webp FROM media WHERE path = ?1 AND thumb_webp IS NOT NULL",
            params![&s],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .optional()
    })
    .flatten()
    .flatten()?;
    crate::thumb_texture::thumb_webp_valid(&b).then_some(b)
}

/// Replace the `media` row after undo of a continue-list removal.
pub fn apply_media_snapshot(s: &MediaRowSnapshot) {
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, duration_sec, time_pos_sec, source_mtime_sec, thumb_webp, thumb_time_pos_sec, audio_aid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
               duration_sec = excluded.duration_sec,
               time_pos_sec = excluded.time_pos_sec,
               source_mtime_sec = excluded.source_mtime_sec,
               thumb_webp = excluded.thumb_webp,
               thumb_time_pos_sec = excluded.thumb_time_pos_sec,
               audio_aid = excluded.audio_aid",
            params![
                &s.path_key,
                s.duration_sec,
                s.time_pos_sec,
                s.source_mtime_sec,
                s.thumb_webp,
                s.thumb_time_pos_sec,
                s.audio_aid
            ],
        )?;
        Ok(())
    });
}

fn now_unix_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// File mtime in whole seconds (for thumb cache key).
pub fn file_mtime_sec(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?;
    let t = m.modified().ok()?;
    t.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}
