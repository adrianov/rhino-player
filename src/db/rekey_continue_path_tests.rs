//! Tests for [super::rekey_history_conn] / [super::rekey_media_conn].

use rusqlite::{params, Connection};

use super::{rekey_history_conn, rekey_media_conn};

fn open_schema(c: &Connection) {
    c.execute_batch(
        "CREATE TABLE history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                last_opened INTEGER NOT NULL
             );
             CREATE TABLE media (
                path TEXT PRIMARY KEY NOT NULL,
                duration_sec REAL,
                time_pos_sec REAL,
                thumb_load_path TEXT
             );",
    )
    .unwrap();
}

#[test]
fn renames_when_target_free() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO history (path, last_opened) VALUES (?, ?)",
        params!["/dl/a.mkv.id.dctmp", 10_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, duration_sec, time_pos_sec, thumb_load_path) VALUES (?, ?, ?, ?)",
        params!["/dl/a.mkv.id.dctmp", 100.0_f64, 40.0_f64, "/dl/a.mkv.id.dctmp"],
    )
    .unwrap();
    assert!(rekey_history_conn(&c, "/dl/a.mkv.id.dctmp", "/dl/a.mkv").unwrap());
    rekey_media_conn(&c, "/dl/a.mkv.id.dctmp", "/dl/a.mkv").unwrap();
    let opened: i64 = c
        .query_row(
            "SELECT last_opened FROM history WHERE path = ?1",
            params!["/dl/a.mkv"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(opened, 10);
    let tpos: f64 = c
        .query_row(
            "SELECT time_pos_sec FROM media WHERE path = ?1",
            params!["/dl/a.mkv"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tpos, 40.0);
    let thumb: String = c
        .query_row(
            "SELECT thumb_load_path FROM media WHERE path = ?1",
            params!["/dl/a.mkv"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(thumb, "/dl/a.mkv");
}

#[test]
fn conflict_keeps_target_history_and_source_media() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO history (path, last_opened) VALUES (?, ?), (?, ?)",
        params!["/dl/a.mkv.id.dctmp", 50_i64, "/dl/a.mkv", 20_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, time_pos_sec) VALUES (?, ?), (?, ?)",
        params!["/dl/a.mkv.id.dctmp", 40.0_f64, "/dl/a.mkv", 5.0_f64],
    )
    .unwrap();
    assert!(rekey_history_conn(&c, "/dl/a.mkv.id.dctmp", "/dl/a.mkv").unwrap());
    rekey_media_conn(&c, "/dl/a.mkv.id.dctmp", "/dl/a.mkv").unwrap();
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let opened: i64 = c
        .query_row(
            "SELECT last_opened FROM history WHERE path = ?1",
            params!["/dl/a.mkv"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(opened, 50);
    let tpos: f64 = c
        .query_row(
            "SELECT time_pos_sec FROM media WHERE path = ?1",
            params!["/dl/a.mkv"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tpos, 40.0);
}

#[test]
fn missing_history_row_is_false() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    assert!(!rekey_history_conn(&c, "/gone.dctmp", "/gone.mkv").unwrap());
}
