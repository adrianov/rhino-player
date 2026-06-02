
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
            |row| {
                Ok(MediaRowSnapshot {
                    path_key: row.get(0)?,
                    duration_sec: row.get(1)?,
                    time_pos_sec: row.get(2)?,
                    source_mtime_sec: row.get(3)?,
                    thumb_webp: row.get(4)?,
                    thumb_time_pos_sec: row.get(5)?,
                    audio_aid: row.get(6)?,
                })
            },
        )
        .optional()
    })
    .flatten()
}

fn stored_thumb_ok(bytes: &[u8]) -> bool {
    crate::thumb_texture::thumb_webp_valid(bytes)
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
    stored_thumb_ok(&b).then_some(b)
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

/// Reuse a thumbnail when the wanted continue position is still near the frame we stored.
const THUMB_TPOS_SKIP_EPS: f64 = 0.5;

/// WebP bytes if we have a thumb for this mtime of the file on disk.
pub fn take_thumb_if_current(path: &str, file_mtime_sec: i64) -> Option<Vec<u8>> {
    with_conn(|c| {
        let row: Option<(Option<Vec<u8>>, Option<i64>)> = c
            .query_row(
                "SELECT thumb_webp, source_mtime_sec FROM media WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(blob), Some(m))) if m == file_mtime_sec && crate::thumb_texture::thumb_webp_valid(&blob) => {
                Some(blob)
            }
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
                "SELECT thumb_webp, source_mtime_sec, thumb_time_pos_sec, thumb_load_path FROM media WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(blob), Some(m), Some(tp), stored_load))
                if m == file_mtime_sec
                    && crate::thumb_texture::thumb_webp_valid(&blob)
                    && tp.is_finite()
                    && (time_pos - tp).abs() < THUMB_TPOS_SKIP_EPS
                    && load_path_matches(load_path, stored_load.as_deref()) =>
            {
                Some(blob)
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
    webp: &[u8],
    source_mtime_sec: i64,
    thumb_time_pos: f64,
    load_path: Option<&str>,
) {
    if webp.is_empty() || !crate::thumb_texture::thumb_webp_valid(webp) {
        if !webp.is_empty() {
            eprintln!(
                "[rhino] grid_thumb reject incomplete blob path={path} bytes={}",
                webp.len()
            );
        }
        return;
    }
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, thumb_webp, source_mtime_sec, thumb_time_pos_sec, thumb_load_path) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
               thumb_webp = excluded.thumb_webp,
               source_mtime_sec = excluded.source_mtime_sec,
               thumb_time_pos_sec = excluded.thumb_time_pos_sec,
               thumb_load_path = excluded.thumb_load_path",
            params![path, webp, source_mtime_sec, thumb_time_pos, load_path],
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
