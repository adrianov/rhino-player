// Path-only media files catalog (`files` table) — feature 34 (partial).
// Tech columns and forget-on-miss stay planned; search / Lucky read [list_file_paths] only.
// [forget_file] drops an unparseable path from files, history, and media.

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

pub(super) fn with_files_conn<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    with_conn(|c| {
        ensure_files_ready(c)?;
        f(c)
    })
}

pub(super) fn unix_now() -> i64 {
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

/// Manual BEGIN/COMMIT keeps a single journal sync without needing &mut Connection.
pub(super) fn with_immediate_tx(
    conn: &Connection,
    f: impl FnOnce(&Connection) -> rusqlite::Result<()>,
) -> rusqlite::Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f(conn) {
        Ok(()) => conn.execute_batch("COMMIT"),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Drop one catalog path and its continue / media rows (unparseable still, gone file).
///
/// Deletes both [history_key] and the exact path string so a post-trash miss (canonicalize
/// fails, macOS `/var` vs `/private/var`) still removes the stored row.
pub fn forget_file(path: &Path) {
    forget_files(&[path.to_path_buf()]);
}

/// Drop many catalog paths in one transaction (search miss packs).
pub fn forget_files(paths: &[PathBuf]) {
    let keys = forget_files_keys(paths);
    if keys.is_empty() {
        if let Some(p) = paths.first() {
            eprintln!("[rhino] db: forget skipped (no key) path={}", p.display());
        }
        return;
    }
    let _ = with_files_conn(|c| {
        with_immediate_tx(c, |c| {
            for key in &keys {
                c.execute("DELETE FROM files WHERE path = ?1", params![key])?;
                c.execute("DELETE FROM history WHERE path = ?1", params![key])?;
                c.execute("DELETE FROM media WHERE path = ?1", params![key])?;
            }
            Ok(())
        })
    });
}

fn forget_files_keys(paths: &[PathBuf]) -> Vec<String> {
    let mut keys = Vec::new();
    for path in paths {
        for k in forget_path_keys(path) {
            if !keys.iter().any(|e| e == &k) {
                keys.push(k);
            }
        }
    }
    keys
}

fn forget_path_keys(path: &Path) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(k) = history_key(path) {
        keys.push(k);
    }
    if let Some(s) = path.to_str() {
        if !keys.iter().any(|k| k == s) {
            keys.push(s.to_string());
        }
    }
    keys
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

#[path = "history_files_catalog_siblings.rs"]
mod siblings;
pub use siblings::{ensure_open_siblings, files_catalog_epoch};
