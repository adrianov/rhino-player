//! Single SQLite file under XDG config: `~/.config/rhino/rhino.sqlite`.
//! Replaces ad-hoc `recent_files.txt` / `durations.txt` / `cache/…/thumbs`. mpv [paths::watch_later] files stay separate (libmpv needs a directory).

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

use rusqlite::{params, Connection, OptionalExtension};

use crate::paths;

const DB_NAME: &str = "rhino.sqlite";
const MAX_HISTORY: i64 = 20;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Open the DB, create tables, one-time legacy import, WAL mode.
pub fn init() {
    let Some(root) = paths::app_config() else {
        eprintln!("[rhino] db: no XDG config dir");
        return;
    };
    let p = root.join(DB_NAME);
    let conn = match Connection::open(&p) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[rhino] db: open {p:?}: {e}");
            return;
        }
    };
    if let Err(e) = conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS history (
             id   INTEGER PRIMARY KEY AUTOINCREMENT,
             path TEXT NOT NULL UNIQUE,
             last_opened INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_history_opened
             ON history (last_opened DESC);
         CREATE TABLE IF NOT EXISTS media (
             path TEXT PRIMARY KEY NOT NULL,
             duration_sec REAL,
             time_pos_sec REAL,
             source_mtime_sec INTEGER,
             thumb_png BLOB
         );
         CREATE TABLE IF NOT EXISTS settings (
             k TEXT PRIMARY KEY NOT NULL,
             v TEXT NOT NULL
         );
         ",
    ) {
        eprintln!("[rhino] db: schema: {e}");
        return;
    }
    if conn
        .execute("ALTER TABLE media ADD COLUMN time_pos_sec REAL", [])
        .is_err()
    {
        // Column already present (e.g. new DB) — ignore.
    }
    if conn
        .execute(
            "ALTER TABLE media ADD COLUMN thumb_time_pos_sec REAL",
            [],
        )
        .is_err()
    {
        // Column already present — ignore.
    }
    if let Err(e) = import_legacy(&conn) {
        eprintln!("[rhino] db: legacy import: {e}");
    }
    if DB.set(Mutex::new(conn)).is_err() {
        eprintln!("[rhino] db: already initialized");
    }
}

fn with_conn<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    let lock = DB.get()?;
    let c = lock.lock().ok()?;
    f(&c).ok()
}

// --- app settings (key-value, small) ---

const K_VOL: &str = "master_volume";
const K_MUTE: &str = "master_mute";

/// Last saved `libmpv` `volume` (0…`volume-max`, typically 0…100) and `mute` from the previous run.
pub fn load_audio() -> (f64, bool) {
    let vol = with_conn(|c| {
        let o = c
            .query_row("SELECT v FROM settings WHERE k = ?1", params![K_VOL], |row| {
                let s: String = row.get(0)?;
                Ok(s)
            })
            .optional()?;
        Ok(
            o.and_then(|s| s.parse::<f64>().ok())
                .filter(|x: &f64| x.is_finite())
                .map(|x| x.clamp(0.0, 200.0))
                .unwrap_or(100.0),
        )
    })
    .unwrap_or(100.0);
    let mute = with_conn(|c| {
        let o = c
            .query_row("SELECT v FROM settings WHERE k = ?1", params![K_MUTE], |row| {
                let s: String = row.get(0)?;
                Ok(s)
            })
            .optional()?;
        Ok(
            o.map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        )
    })
    .unwrap_or(false);
    (vol, mute)
}

/// Persist for the next app launch. Safe to call from the quit path before [commit_quit].
pub fn save_audio(volume: f64, muted: bool) {
    if !volume.is_finite() {
        return;
    }
    let v = volume.clamp(0.0, 200.0);
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_VOL, format!("{v:.4}")],
        )?;
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_MUTE, if muted { "1" } else { "0" }],
        )?;
        Ok(())
    });
}

// --- history ---

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

pub fn remove_history(path: &Path) {
    let s = std::fs::canonicalize(path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string));
    let Some(s) = s else { return };
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

/// Last playback time (libmpv `time-pos`, seconds) for the recent bar; complements watch_later.
pub fn load_time_pos_map() -> HashMap<String, f64> {
    with_conn(|c| {
        let mut s = c.prepare("SELECT path, time_pos_sec FROM media WHERE time_pos_sec IS NOT NULL")?;
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

/// Do not re-capture a quit-time screenshot if the on-disk file is unchanged and
/// [time_pos] is still within this many seconds of the frame we stored. (See [set_thumb] `thumb_time_pos_sec`.)
const THUMB_TPOS_SKIP_EPS: f64 = 0.5;

/// Returns `true` when an existing DB thumbnail is for the same file revision and the same
/// `time-pos` (within [THUMB_TPOS_SKIP_EPS]) so a new [screenshot-to-file] is unnecessary.
pub fn should_skip_quit_thumb(path: &str, file_mtime_sec: i64, time_pos: f64) -> bool {
    if !time_pos.is_finite() || time_pos < 0.0 {
        return false;
    }
    type Row = (Option<Vec<u8>>, Option<i64>, Option<f64>);
    with_conn(|c| {
        let r: Option<Row> = c
            .query_row(
                "SELECT thumb_png, source_mtime_sec, thumb_time_pos_sec FROM media WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        Ok(match r {
            Some((Some(b), Some(m), Some(tp)))
                if !b.is_empty() && m == file_mtime_sec && tp.is_finite() =>
            {
                (time_pos - tp).abs() < THUMB_TPOS_SKIP_EPS
            }
            _ => false,
        })
    })
    .unwrap_or(false)
}

/// PNG bytes if we have a thumb for this mtime of the file on disk.
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

/// `thumb_time_pos` is the libmpv [time-pos] (seconds) of the frame in [png] (the stored raster).
pub fn set_thumb(path: &str, png: &[u8], source_mtime_sec: i64, thumb_time_pos: f64) {
    if png.is_empty() {
        return;
    }
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, thumb_png, source_mtime_sec, thumb_time_pos_sec) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               thumb_png = excluded.thumb_png,
               source_mtime_sec = excluded.source_mtime_sec,
               thumb_time_pos_sec = excluded.thumb_time_pos_sec",
            params![path, png, source_mtime_sec, thumb_time_pos],
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

fn import_legacy(c: &Connection) -> rusqlite::Result<()> {
    let Some(cfg) = paths::app_config() else {
        return Ok(());
    };
    let now = now_unix_ms();
    let recent = cfg.join("recent_files.txt");
    if recent.is_file() {
        if let Ok(f) = std::fs::File::open(&recent) {
            let lines: Vec<String> = BufReader::new(f)
                .lines()
                .map_while(|l| l.ok())
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            for (i, t) in lines.iter().enumerate() {
                if Path::new(t).is_file() {
                    let op = now - (i as i64) * 1_000;
                    c.execute(
                        "INSERT INTO history (path, last_opened) VALUES (?1, ?2)
                     ON CONFLICT(path) DO UPDATE SET
                       last_opened = MAX(history.last_opened, excluded.last_opened)",
                        params![t, op],
                    )?;
                }
            }
            c.execute(
                "DELETE FROM history WHERE id NOT IN (
                 SELECT id FROM (SELECT id FROM history ORDER BY last_opened DESC LIMIT ?1)
             )",
                params![MAX_HISTORY],
            )?;
            let _ = std::fs::rename(&recent, cfg.join("recent_files.txt.migrated"));
        }
    }
    let durs = cfg.join("durations.txt");
    if durs.is_file() {
        if let Ok(f) = std::fs::File::open(&durs) {
            for line in BufReader::new(f).lines().map_while(|l| l.ok()) {
                if let Some((a, b)) = line.split_once('\t') {
                    let a = a.trim();
                    if let Ok(sec) = b.trim().parse::<f64>() {
                        if sec.is_finite() && sec > 0.0 {
                            c.execute(
                                "INSERT INTO media (path, duration_sec) VALUES (?1, ?2)
                             ON CONFLICT(path) DO UPDATE SET duration_sec = excluded.duration_sec",
                                params![a, sec],
                            )?;
                        }
                    }
                }
            }
            let _ = std::fs::rename(&durs, cfg.join("durations.txt.migrated"));
        }
    }
    Ok(())
}
