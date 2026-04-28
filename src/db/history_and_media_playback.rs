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
    let Some(s) = std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
    else {
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

fn history_key(path: &Path) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .or_else(|| {
            if path.is_absolute() {
                path.to_str().map(str::to_string)
            } else {
                None
            }
        })
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
    let Some(s) = std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
    else {
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

/// Store [duration_sec] and [time_pos_sec] (seconds) for a local file. Used on file switch and close.
pub fn set_playback(path: &Path, duration_sec: f64, time_pos_sec: f64) {
    if !duration_sec.is_finite() || duration_sec <= 0.0 {
        return;
    }
    if !time_pos_sec.is_finite() || time_pos_sec < 0.0 {
        return;
    }
    let Some(s) = std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
    else {
        return;
    };
    let t = time_pos_sec.min(duration_sec);
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
