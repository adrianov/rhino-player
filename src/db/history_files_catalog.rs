// Path-only media files catalog (`files` table) — feature 34 (partial).
// Tech columns and forget-on-miss stay planned; neighbour search seeds from [list_file_paths].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use super::{history_key, with_conn};

static FILES_READY: AtomicBool = AtomicBool::new(false);

fn migrate_files_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
             path TEXT PRIMARY KEY NOT NULL,
             discovered_at INTEGER NOT NULL
         );",
    )
    .map_err(|e| {
        eprintln!("[rhino] db: files schema: {e}");
        e
    })?;
    conn.execute(
        "INSERT OR IGNORE INTO files (path, discovered_at)
         SELECT path, ?1 FROM history
         UNION
         SELECT path, ?1 FROM media",
        params![unix_now()],
    )
    .map_err(|e| {
        eprintln!("[rhino] db: files seed: {e}");
        e
    })?;
    Ok(())
}

fn ensure_files_ready(conn: &Connection) -> rusqlite::Result<()> {
    if FILES_READY.load(Ordering::Relaxed) {
        return Ok(());
    }
    migrate_files_table(conn)?;
    FILES_READY.store(true, Ordering::Relaxed);
    Ok(())
}

fn with_files_conn<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    with_conn(|c| {
        ensure_files_ready(c)?;
        f(c)
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Insert-or-ignore one catalog path (entity key when possible).
pub fn ensure_file(path: &Path) {
    let Some(key) = history_key(path) else {
        return;
    };
    let _ = with_files_conn(|c| {
        c.execute(
            "INSERT OR IGNORE INTO files (path, discovered_at) VALUES (?1, ?2)",
            params![key, unix_now()],
        )?;
        Ok(())
    });
}

/// Register many paths in one transaction (neighbour-folder scan discoveries).
pub fn ensure_files(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    let now = unix_now();
    let _ = with_files_conn(|c| insert_files_tx(c, paths, now));
}

fn insert_files_tx(conn: &Connection, paths: &[PathBuf], now: i64) -> rusqlite::Result<()> {
    // Manual BEGIN/COMMIT keeps a single journal sync without needing &mut Connection.
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let mut stmt =
            conn.prepare("INSERT OR IGNORE INTO files (path, discovered_at) VALUES (?1, ?2)")?;
        for p in paths {
            if let Some(key) = history_key(p) {
                stmt.execute(params![key, now])?;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("COMMIT"),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Every catalog path (order stable by path text). Empty when the DB is unavailable.
pub fn list_file_paths() -> Vec<PathBuf> {
    with_files_conn(|c| {
        let mut stmt = c.prepare("SELECT path FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok().map(PathBuf::from)).collect())
    })
    .unwrap_or_default()
}
