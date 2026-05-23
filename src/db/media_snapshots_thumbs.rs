
/// Full `media` row for undo after “remove from list”; [path_key] is the same as [history_key] strings.
#[derive(Debug, Clone)]
pub struct MediaRowSnapshot {
    pub path_key: String,
    pub duration_sec: Option<f64>,
    pub time_pos_sec: Option<f64>,
    pub source_mtime_sec: Option<i64>,
    pub thumb_png: Option<Vec<u8>>,
    pub thumb_time_pos_sec: Option<f64>,
    pub audio_aid: Option<i64>,
}

/// Read the row for this path, if any.
pub fn snapshot_media_row(path: &Path) -> Option<MediaRowSnapshot> {
    let path_key = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT path, duration_sec, time_pos_sec, source_mtime_sec, thumb_png, thumb_time_pos_sec, audio_aid
             FROM media WHERE path = ?1",
            params![&path_key],
            |row| {
                Ok(MediaRowSnapshot {
                    path_key: row.get(0)?,
                    duration_sec: row.get(1)?,
                    time_pos_sec: row.get(2)?,
                    source_mtime_sec: row.get(3)?,
                    thumb_png: row.get(4)?,
                    thumb_time_pos_sec: row.get(5)?,
                    audio_aid: row.get(6)?,
                })
            },
        )
        .optional()
    })
    .flatten()
}

/// PNG bytes for this entity path key when present (no mtime check).
pub fn stored_thumb_png(path: &Path) -> Option<Vec<u8>> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT thumb_png FROM media WHERE path = ?1 AND thumb_png IS NOT NULL",
            params![&s],
            |row| row.get::<_, Option<Vec<u8>>>(0),
        )
        .optional()
    })
    .flatten()
    .flatten()
}

/// Replace the `media` row after undo of a continue-list removal.
pub fn apply_media_snapshot(s: &MediaRowSnapshot) {
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, duration_sec, time_pos_sec, source_mtime_sec, thumb_png, thumb_time_pos_sec, audio_aid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
               duration_sec = excluded.duration_sec,
               time_pos_sec = excluded.time_pos_sec,
               source_mtime_sec = excluded.source_mtime_sec,
               thumb_png = excluded.thumb_png,
               thumb_time_pos_sec = excluded.thumb_time_pos_sec,
               audio_aid = excluded.audio_aid",
            params![
                &s.path_key,
                s.duration_sec,
                s.time_pos_sec,
                s.source_mtime_sec,
                s.thumb_png,
                s.thumb_time_pos_sec,
                s.audio_aid
            ],
        )?;
        Ok(())
    });
}

/// Reuse a thumbnail when the wanted continue position is still near the frame we stored.
const THUMB_TPOS_SKIP_EPS: f64 = 0.5;

/// PNG/JPEG bytes if we have a thumb for this mtime of the file on disk.
pub fn take_thumb_if_current(path: &str, file_mtime_sec: i64) -> Option<Vec<u8>> {
    with_conn(|c| {
        let row: Option<(Option<Vec<u8>>, Option<i64>)> = c
            .query_row(
                "SELECT thumb_png, source_mtime_sec FROM media WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(png), Some(m))) if m == file_mtime_sec => Some(png),
            _ => None,
        })
    })
    .flatten()
}

type ThumbRow = (Option<Vec<u8>>, Option<i64>, Option<f64>, Option<String>);

/// Thumb bytes if the file mtime matches, the stored frame is near the wanted continue time,
/// and the cached chapter load path matches (DVD multi-VOB titles).
pub fn take_thumb_if_fresh(
    path: &str,
    file_mtime_sec: i64,
    time_pos: f64,
    load_path: Option<&str>,
) -> Option<Vec<u8>> {
    if !time_pos.is_finite() || time_pos < 0.0 {
        return take_thumb_if_current(path, file_mtime_sec);
    }
    with_conn(|c| {
        let row: Option<ThumbRow> = c
            .query_row(
                "SELECT thumb_png, source_mtime_sec, thumb_time_pos_sec, thumb_load_path FROM media WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(png), Some(m), Some(tp), stored_load))
                if m == file_mtime_sec
                    && tp.is_finite()
                    && (time_pos - tp).abs() < THUMB_TPOS_SKIP_EPS
                    && load_path_matches(load_path, stored_load.as_deref()) =>
            {
                Some(png)
            }
            _ => None,
        })
    })
    .flatten()
}

fn load_path_matches(want: Option<&str>, stored: Option<&str>) -> bool {
    use std::path::Path;
    match (want, stored) {
        (None, None) => true,
        (Some(w), Some(s)) if w == s => true,
        (Some(w), Some(s)) => crate::video_ext::paths_same_file(Path::new(w), Path::new(s)),
        (Some(_), None) => false,
        (None, Some(_)) => false,
    }
}

/// `thumb_time_pos` is whole-title seconds; [load_path] is the chapter file mpv loaded for the frame.
pub fn set_thumb(
    path: &str,
    png: &[u8],
    source_mtime_sec: i64,
    thumb_time_pos: f64,
    load_path: Option<&str>,
) {
    if png.is_empty() {
        return;
    }
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, thumb_png, source_mtime_sec, thumb_time_pos_sec, thumb_load_path) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
               thumb_png = excluded.thumb_png,
               source_mtime_sec = excluded.source_mtime_sec,
               thumb_time_pos_sec = excluded.thumb_time_pos_sec,
               thumb_load_path = excluded.thumb_load_path",
            params![path, png, source_mtime_sec, thumb_time_pos, load_path],
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

/// File mtime in whole seconds (for cache key); matches prior PNG cache behavior.
pub fn file_mtime_sec(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?;
    let t = m.modified().ok()?;
    t.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}
