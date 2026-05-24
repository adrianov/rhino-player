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
        if history_key(&crate::playback_entity::db_path_for(&p)).as_deref() == Some(target.as_str()) {
            delete_history_stored_path(&p);
        }
    }
    remove_history(&crate::playback_entity::db_path_for(path));
}

// --- media (duration + thumb) ---

/// Last-known duration for progress on the recent grid. Keys: canonical path strings.
pub fn load_duration_map() -> HashMap<String, f64> {
    with_conn(|c| {
        let mut s = c.prepare("SELECT path, duration_sec FROM media WHERE duration_sec IS NOT NULL AND duration_sec > 0")?;
        let m = s.query_map([], |row| {
            let p: String = row.get(0)?;
            let d: f64 = row.get(1)?;
            Ok((p, d))
        })?;
        Ok(m.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
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

/// Resume position (seconds) for one file. Used by `loadfile` to pass `start=<sec>`.
/// Same path key as [remove_history] / [clear_resume_position].
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

/// Near-start mpv reads during warm preload must not clobber a stored resume (see `media_probe::NEAR_END`).
const MIN_PERSIST_RESUME_SEC: f64 = 3.0;

/// Store [duration_sec] and [time_pos_sec] (seconds) for a local file. Used on file switch and close.
pub fn set_playback(path: &Path, duration_sec: f64, time_pos_sec: f64) {
    if !duration_sec.is_finite() || duration_sec <= 0.0 {
        return;
    }
    if !time_pos_sec.is_finite() || time_pos_sec < 0.0 {
        return;
    }
    let t = time_pos_sec.min(duration_sec);
    if t < MIN_PERSIST_RESUME_SEC && resume_pos(path).is_some() {
        set_duration(path, duration_sec);
        return;
    }
    if t < 1.0 {
        set_duration(path, duration_sec);
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

/// Store the chosen audio track id immediately so SIGTERM / `kill` does not reset it.
pub fn set_audio_aid(path: &Path, aid: i64) {
    if aid <= 0 {
        return;
    }
    let Some(s) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, audio_aid) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET audio_aid = excluded.audio_aid",
            params![&s, aid],
        )?;
        Ok(())
    });
}

pub fn load_audio_aid(path: &Path) -> Option<i64> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT audio_aid FROM media WHERE path = ?1",
            params![&s],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
    })
    .flatten()
    .flatten()
    .filter(|aid| *aid > 0)
}

/// Last hand-picked subtitle on this playback entity (`sid` + optional DVD IFO slot).
pub fn set_sub_track(path: &Path, sid: i64, ifo_slot: Option<u8>) {
    if sid <= 0 {
        return;
    }
    let Some(s) = history_key(path) else {
        return;
    };
    let slot = ifo_slot.map(i64::from);
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, sub_sid, sub_ifo_slot) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
               sub_sid = excluded.sub_sid,
               sub_ifo_slot = excluded.sub_ifo_slot",
            params![&s, sid, slot],
        )?;
        Ok(())
    });
}

pub fn load_sub_track(path: &Path) -> Option<(i64, Option<u8>)> {
    let s = history_key(path)?;
    with_conn(|c| {
        c.query_row(
            "SELECT sub_sid, sub_ifo_slot FROM media WHERE path = ?1",
            params![&s],
            |row| {
                let sid: Option<i64> = row.get(0)?;
                let slot: Option<i64> = row.get(1)?;
                Ok((sid, slot))
            },
        )
        .optional()
    })
    .flatten()
    .and_then(|(sid, slot)| {
        let sid = sid.filter(|n| *n > 0)?;
        let slot = slot.and_then(|n| u8::try_from(n).ok());
        Some((sid, slot))
    })
}

/// Clear stored resume so the next open starts from 0.
/// Uses the same path key as [remove_history] so deleted-on-disk files still match DB rows.
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
