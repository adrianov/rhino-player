//! Rename persistence (feature 37): `files` + optional `history` / `media` in one transaction.

use std::cell::RefCell;
use std::path::Path;

use rusqlite::{params, Connection};

use super::files_catalog;
use super::{history_key, rekey_history_conn, rekey_media_conn};

struct RenameKeys {
    from_hist: String,
    from_file: String,
    to_s: String,
}

/// After an on-disk rename (continue / search / Lucky): rekey `files`, and `history` /
/// `media` when present, in one transaction. Does not require a history row.
pub fn rekey_renamed_path(from: &Path, to: &Path) -> Result<(), String> {
    let keys = rename_keys(from, to)?;
    if keys.from_hist == keys.to_s && keys.from_file == keys.to_s {
        return Ok(());
    }
    commit_rename_rekey(&keys)
}

fn rename_keys(from: &Path, to: &Path) -> Result<RenameKeys, String> {
    let from_hist = from
        .to_str()
        .ok_or_else(|| "Invalid file path.".to_string())?
        .to_owned();
    let from_file = history_key(from).unwrap_or_else(|| from_hist.clone());
    let to_s = history_key(to).ok_or_else(|| "Invalid destination path.".to_string())?;
    Ok(RenameKeys {
        from_hist,
        from_file,
        to_s,
    })
}

/// `with_conn` maps SQL errors to `None`; capture the message before that happens.
fn commit_rename_rekey(keys: &RenameKeys) -> Result<(), String> {
    let fail = RefCell::new(None);
    match (
        files_catalog::with_files_conn(|c| capture_rename_tx(c, keys, &fail)),
        fail.into_inner(),
    ) {
        (_, Some(e)) => Err(format!("Could not update the library ({e}).")),
        (Some(()), None) => Ok(()),
        (None, None) => Err("Library database unavailable.".into()),
    }
}

fn capture_rename_tx(
    c: &Connection,
    keys: &RenameKeys,
    fail: &RefCell<Option<String>>,
) -> rusqlite::Result<()> {
    rename_rekey_tx(c, keys).map_err(|e| {
        *fail.borrow_mut() = Some(e.to_string());
        e
    })
}

fn rename_rekey_tx(c: &Connection, keys: &RenameKeys) -> rusqlite::Result<()> {
    files_catalog::with_immediate_tx(c, |c| {
        rekey_files_in_tx(c, &keys.from_file, &keys.to_s)?;
        if keys.from_hist != keys.to_s {
            rekey_history_media(c, &keys.from_hist, &keys.to_s)?;
        }
        Ok(())
    })
}

fn rekey_history_media(c: &Connection, from: &str, to: &str) -> rusqlite::Result<()> {
    if rekey_history_conn(c, from, to)? {
        rekey_media_conn(c, from, to)?;
    } else {
        // Search / Lucky may have media without a continue-history row.
        rekey_media_conn(c, from, to)?;
    }
    Ok(())
}

fn rekey_files_in_tx(c: &Connection, from: &str, to: &str) -> rusqlite::Result<()> {
    if from == to {
        return Ok(());
    }
    if files_row_exists(c, from)? {
        rekey_files_conn(c, from, to)?;
        return Ok(());
    }
    c.execute(
        "INSERT OR IGNORE INTO files (path, discovered_at) VALUES (?1, ?2)",
        params![to, files_catalog::unix_now()],
    )?;
    Ok(())
}

fn rekey_files_conn(c: &Connection, from: &str, to: &str) -> rusqlite::Result<bool> {
    if !files_row_exists(c, from)? {
        return Ok(false);
    }
    if files_row_exists(c, to)? {
        c.execute("DELETE FROM files WHERE path = ?1", params![from])?;
    } else {
        c.execute(
            "UPDATE files SET path = ?1 WHERE path = ?2",
            params![to, from],
        )?;
    }
    Ok(true)
}

fn files_row_exists(c: &Connection, path: &str) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM files WHERE path = ?1)",
        params![path],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::rekey_files_conn;
    use rusqlite::{params, Connection};

    fn open_files() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE files (
                 path TEXT PRIMARY KEY NOT NULL,
                 discovered_at INTEGER NOT NULL
             );",
        )
        .unwrap();
        c
    }

    #[test]
    fn files_row_moves_without_history() {
        let c = open_files();
        c.execute(
            "INSERT INTO files (path, discovered_at) VALUES (?, 7)",
            params!["/lib/clip.mkv"],
        )
        .unwrap();
        assert!(rekey_files_conn(&c, "/lib/clip.mkv", "/lib/renamed.mkv").unwrap());
        let discovered: i64 = c
            .query_row(
                "SELECT discovered_at FROM files WHERE path = ?1",
                params!["/lib/renamed.mkv"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(discovered, 7);
    }
}
