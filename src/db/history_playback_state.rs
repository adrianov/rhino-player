// Resume position, playback persistence, and per-track (audio/sub) storage.
// Split-out unit of the flat `db` module; public paths (`db::set_playback`, …) are re-exported.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, OptionalExtension};

use super::{history_key, with_conn};

/// Near-start mpv reads during warm preload must not clobber a stored resume (see `media_probe::NEAR_END`).
const MIN_PERSIST_RESUME_SEC: f64 = 3.0;

/// Resume position (seconds) for one file. Used by `loadfile` to pass `start=<sec>`.
/// Same path key as [super::remove_history] / [clear_resume_position].
pub fn resume_pos(path: &Path) -> Option<f64> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT time_pos_sec FROM media WHERE path = ?1",
            params![&s],
            |row| row.get::<_, Option<f64>>(0),
        )
        .optional()
    })
    .flatten()
    .flatten()
    .filter(|t| t.is_finite() && *t > 0.0)
}

/// Last playback time (libmpv `time-pos`, seconds) for the recent bar.
pub fn load_time_pos_map() -> HashMap<String, f64> {
    with_conn(|c| {
        let mut s =
            c.prepare("SELECT path, time_pos_sec FROM media WHERE time_pos_sec IS NOT NULL")?;
        let m = s.query_map([], |row| {
            let p: String = row.get(0)?;
            let t: f64 = row.get(1)?;
            Ok((p, t))
        })?;
        Ok(m.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

/// Duration-only fallback: near-start reads and sub-second positions must not clobber a stored resume.
fn persist_duration_only(path: &Path, duration_sec: f64, t: f64) -> bool {
    if t < MIN_PERSIST_RESUME_SEC && resume_pos(path).is_some() {
        super::set_duration(path, duration_sec);
        return true;
    }
    if t < 1.0 {
        super::set_duration(path, duration_sec);
        return true;
    }
    false
}

/// Store [duration_sec] and [time_pos_sec] (seconds) for a local file. Used on file switch and close.
pub fn set_playback(path: &Path, duration_sec: f64, time_pos_sec: f64) {
    if !duration_sec.is_finite()
        || duration_sec <= 0.0
        || !time_pos_sec.is_finite()
        || time_pos_sec < 0.0
    {
        return;
    }
    let t = time_pos_sec.min(duration_sec);
    if persist_duration_only(path, duration_sec, t) {
        return;
    }
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, duration_sec, time_pos_sec) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
               duration_sec = excluded.duration_sec,
               time_pos_sec = excluded.time_pos_sec",
            params![&s, duration_sec, t],
        )?;
        Ok(())
    });
    crate::playback_entity::purge_extra_db_rows(path);
}

/// Store the chosen audio track immediately so SIGTERM / `kill` does not reset it.
pub fn set_audio_track(path: &Path, aid: i64, ifo_slot: Option<u8>) {
    if aid <= 0 {
        return;
    }
    store_track_slot("audio_aid, audio_ifo_slot", path, aid, ifo_slot);
}

/// Last hand-picked subtitle on this playback entity (`sid` + optional DVD IFO slot).
pub fn set_sub_track(path: &Path, sid: i64, ifo_slot: Option<u8>) {
    if sid <= 0 {
        return;
    }
    store_track_slot("sub_sid, sub_ifo_slot", path, sid, ifo_slot);
}

/// Shared upsert for [set_audio_track] / [set_sub_track]; `columns` is `<id_col>, <slot_col>`.
fn store_track_slot(columns: &str, path: &Path, id: i64, ifo_slot: Option<u8>) {
    let Some(s) = history_key(path) else {
        return;
    };
    let slot = ifo_slot.map(i64::from);
    let _ = with_conn(|c| {
        c.execute(
            &format!(
                "INSERT INTO media (path, {columns}) VALUES (?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET
                   {columns} = excluded.{columns}"
            ),
            params![&s, id, slot],
        )?;
        Ok(())
    });
}

/// (`Option<i64>` id, `Option<i64>` IFO slot) row mapping shared by both track loaders.
fn track_pair(row: &rusqlite::Row) -> rusqlite::Result<(Option<i64>, Option<i64>)> {
    let id: Option<i64> = row.get(0)?;
    let slot: Option<i64> = row.get(1)?;
    Ok((id, slot))
}

/// Reject non-positive ids and out-of-`u8` slots; shared by both track loaders.
fn positive_id_with_slot(pair: (Option<i64>, Option<i64>)) -> Option<(i64, Option<u8>)> {
    let id = pair.0.filter(|n| *n > 0)?;
    Some((id, pair.1.and_then(|n| u8::try_from(n).ok())))
}

pub fn load_audio_track(path: &Path) -> Option<(i64, Option<u8>)> {
    load_track_slot("audio_aid, audio_ifo_slot", path).and_then(positive_id_with_slot)
}

pub fn load_sub_track(path: &Path) -> Option<(i64, Option<u8>)> {
    load_track_slot("sub_sid, sub_ifo_slot", path).and_then(positive_id_with_slot)
}

/// Shared read for [load_audio_track] / [load_sub_track]; `columns` is `<id_col>, <slot_col>`.
fn load_track_slot(columns: &str, path: &Path) -> Option<(Option<i64>, Option<i64>)> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            &format!("SELECT {columns} FROM media WHERE path = ?1"),
            params![&s],
            track_pair,
        )
        .optional()
    })
    .flatten()
}

/// Clear stored resume so the next open starts from 0.
/// Uses the same path key as [super::remove_history] so deleted-on-disk files still match DB rows.
pub fn clear_resume_position(path: &Path) {
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "UPDATE media SET time_pos_sec = NULL WHERE path = ?1",
            params![&s],
        )?;
        Ok(())
    });
}
