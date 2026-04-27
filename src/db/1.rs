
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

use rusqlite::{params, Connection, OptionalExtension};

use crate::paths;

const DB_NAME: &str = "rhino.sqlite";
const MAX_HISTORY: i64 = 20;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Open the DB, create current tables, and enable WAL mode.
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
             thumb_png BLOB,
             thumb_time_pos_sec REAL,
             audio_aid INTEGER
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
const K_AUDIO_TRACK_NAME: &str = "audio_track_name";

/// Last saved `libmpv` `volume` (0…`volume-max`, typically 0…100) and `mute` from the previous run.
pub fn load_audio() -> (f64, bool) {
    let vol = with_conn(|c| {
        let o = c
            .query_row(
                "SELECT v FROM settings WHERE k = ?1",
                params![K_VOL],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(s)
                },
            )
            .optional()?;
        Ok(o.and_then(|s| s.parse::<f64>().ok())
            .filter(|x: &f64| x.is_finite())
            .map(|x| x.clamp(0.0, 200.0))
            .unwrap_or(100.0))
    })
    .unwrap_or(100.0);
    let mute = with_conn(|c| {
        let o = c
            .query_row(
                "SELECT v FROM settings WHERE k = ?1",
                params![K_MUTE],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(s)
                },
            )
            .optional()?;
        Ok(o.map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false))
    })
    .unwrap_or(false);
    (vol, mute)
}

/// Persist for the next app launch. Safe to call from the quit path before [commit_quit].
const K_SEEK_BAR_PREVIEW: &str = "seek_bar_preview";

/// [docs/features/18-thumbnail-preview.md] — `true` by default.
pub fn load_seek_bar_preview() -> bool {
    with_conn(|c| {
        let o = c
            .query_row(
                "SELECT v FROM settings WHERE k = ?1",
                params![K_SEEK_BAR_PREVIEW],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(s)
                },
            )
            .optional()?;
        Ok(o.map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(true))
    })
    .unwrap_or(true)
}

pub fn save_seek_bar_preview(on: bool) {
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_SEEK_BAR_PREVIEW, if on { "1" } else { "0" }],
        )?;
        Ok(())
    });
}

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

pub fn load_audio_track_name() -> Option<String> {
    get_setting_str(K_AUDIO_TRACK_NAME)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn save_audio_track_name(name: &str) {
    let s = name.trim();
    if s.is_empty() {
        return;
    }
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_AUDIO_TRACK_NAME, s],
        )?;
        Ok(())
    });
}

// --- video: optional VapourSynth ~60 fps vf (see docs/features/26-sixty-fps-motion.md) ---

/// Current key; bool `0`/`1`.
const K_VIDEO_SMOOTH_60: &str = "video_smooth_60";
const K_VIDEO_VS: &str = "video_vs_path";
const K_VIDEO_MVTOOLS_LIB: &str = "video_mvtools_lib";

#[derive(Debug, Clone)]
pub struct VideoPrefs {
    /// When set: add mpv `vf=vapoursynth` with [vs_path] or bundled `.vpy` (no `display-resample`).
    pub smooth_60: bool,
    /// Absolute path to a `.vpy` for mpv’s `vapoursynth` filter, or empty for bundled script.
    pub vs_path: String,
    /// Cached absolute path to `libmvtools.so` after a successful find; skipped on next call if still a file.
    pub mvtools_lib: String,
}

impl Default for VideoPrefs {
    fn default() -> Self {
        Self {
            // Bundled `rhino_60_mvtools.vpy` when `video_vs_path` is empty; see paths.rs
            smooth_60: true,
            vs_path: String::new(),
            mvtools_lib: String::new(),
        }
    }
}

