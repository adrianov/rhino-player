// Thumbnail reuse/store rules on `media` rows (mtime key, continue-frame proximity, chapter load path).
// Split-out unit of the flat `db` module; public paths (`db::take_thumb_if_fresh`, …) are re-exported.

use rusqlite::{params, OptionalExtension};

use super::with_conn;

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
        Ok(current_thumb(row, file_mtime_sec))
    })
    .flatten()
}

/// Blob when the stored mtime matches the file and the blob is a complete WebP.
fn current_thumb(
    row: Option<(Option<Vec<u8>>, Option<i64>)>,
    file_mtime_sec: i64,
) -> Option<Vec<u8>> {
    match row {
        Some((Some(blob), Some(m)))
            if m == file_mtime_sec && crate::thumb_texture::thumb_webp_valid(&blob) =>
        {
            Some(blob)
        }
        _ => None,
    }
}

type ThumbRow = (Option<Vec<u8>>, Option<i64>, Option<f64>, Option<String>);

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
        Ok(fresh_thumb(
            thumb_row(c, path)?,
            file_mtime_sec,
            time_pos,
            load_path,
        ))
    })
    .flatten()
}

/// Fetch `(thumb_webp, source_mtime_sec, thumb_time_pos_sec, thumb_load_path)` for one path.
fn thumb_row(c: &rusqlite::Connection, path: &str) -> rusqlite::Result<Option<ThumbRow>> {
    c.query_row(
        "SELECT thumb_webp, source_mtime_sec, thumb_time_pos_sec, thumb_load_path FROM media WHERE path = ?1",
        params![path],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()
}

/// Blob when the mtime matches, the stored frame is near [time_pos], and the chapter load path matches.
fn fresh_thumb(
    row: Option<ThumbRow>,
    file_mtime_sec: i64,
    time_pos: f64,
    load_path: Option<&str>,
) -> Option<Vec<u8>> {
    match row {
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
    }
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
