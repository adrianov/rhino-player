use std::sync::{Mutex, OnceLock};

use rusqlite::{params, Connection, OptionalExtension};

use crate::paths;

const DB_NAME: &str = "rhino.sqlite";
const MAX_HISTORY: i64 = 20;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

// Split-out units of the flat `db` module; public paths (`db::load_audio`, …) stay stable.
mod settings_kv {
    include!("connection_settings_kv.rs");
}
pub use settings_kv::*;

mod video_prefs_store {
    include!("connection_video_prefs_store.rs");
}
pub use video_prefs_store::*;

/// One-shot migration marker key (see [video_prefs_store::migrate_smooth_max_area_legacy_adaptive_pollution]).
const K_SMOOTH_MAX_AREA_LEGACY_ADAPTIVE_RESET_V1: &str = "smooth_max_area_legacy_adaptive_reset_v1";

/// Open the DB, create current tables, enable WAL mode, and publish the global handle.
pub fn init() {
    let Some(conn) = open_conn() else {
        return;
    };
    if DB.set(Mutex::new(conn)).is_err() {
        eprintln!("[rhino] db: already initialized");
    }
}

const SCHEMA_SQL: &str = "PRAGMA foreign_keys = ON;
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
             thumb_webp BLOB,
             thumb_time_pos_sec REAL,
             audio_aid INTEGER
         );
         CREATE TABLE IF NOT EXISTS settings (
             k TEXT PRIMARY KEY NOT NULL,
             v TEXT NOT NULL
         );
         ";

/// Open the DB file, create current tables, and run the idempotent migrations.
fn open_conn() -> Option<Connection> {
    let Some(root) = paths::app_config() else {
        eprintln!("[rhino] db: no XDG config dir");
        return None;
    };
    let p = root.join(DB_NAME);
    let conn = match Connection::open(&p) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[rhino] db: open {p:?}: {e}");
            return None;
        }
    };
    if let Err(e) = conn.execute_batch(SCHEMA_SQL) {
        eprintln!("[rhino] db: schema: {e}");
        return None;
    }
    run_column_migrations(&conn);
    Some(conn)
}

/// Column/pref migrations in their historical order; each is idempotent.
fn run_column_migrations(conn: &Connection) {
    migrate_media_decode_columns(conn);
    migrate_media_source_fps_column(conn);
    migrate_media_thumb_load_path(conn);
    migrate_media_thumb_webp_column(conn);
    migrate_media_sub_track_columns(conn);
    migrate_media_audio_ifo_slot(conn);
    video_prefs_store::migrate_legacy_smooth_max_area_round_mil(conn);
    video_prefs_store::migrate_smooth_max_area_legacy_adaptive_pollution(conn);
}

/// Add per-file decode size + ME budget columns (idempotent on existing DBs).
fn migrate_media_decode_columns(conn: &Connection) {
    for sql in [
        "ALTER TABLE media ADD COLUMN decode_w INTEGER",
        "ALTER TABLE media ADD COLUMN decode_h INTEGER",
        "ALTER TABLE media ADD COLUMN smooth_me_budget_px2 INTEGER",
    ] {
        let _ = conn.execute(sql, rusqlite::params![]);
    }
    let _ = conn.execute(
        "ALTER TABLE media RENAME COLUMN smooth_me_budget_saved_ms TO smooth_me_budget_updated_at",
        rusqlite::params![],
    );
    let _ = conn.execute(
        "ALTER TABLE media ADD COLUMN smooth_me_budget_updated_at INTEGER",
        rusqlite::params![],
    );
}

fn migrate_media_source_fps_column(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE media ADD COLUMN source_fps_hz REAL",
        rusqlite::params![],
    );
}

fn migrate_media_thumb_load_path(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE media ADD COLUMN thumb_load_path TEXT",
        rusqlite::params![],
    );
}

/// Idempotent rename for DBs created before `thumb_webp` column name.
fn migrate_media_thumb_webp_column(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE media RENAME COLUMN thumb_png TO thumb_webp",
        rusqlite::params![],
    );
}

fn migrate_media_sub_track_columns(conn: &Connection) {
    for sql in [
        "ALTER TABLE media ADD COLUMN sub_sid INTEGER",
        "ALTER TABLE media ADD COLUMN sub_ifo_slot INTEGER",
    ] {
        let _ = conn.execute(sql, rusqlite::params![]);
    }
}

fn migrate_media_audio_ifo_slot(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE media ADD COLUMN audio_ifo_slot INTEGER",
        rusqlite::params![],
    );
}

pub(crate) fn with_conn<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    let lock = DB.get()?;
    let c = lock.lock().ok()?;
    f(&c).ok()
}
