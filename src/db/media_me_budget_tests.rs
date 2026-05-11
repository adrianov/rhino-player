//! Tests [super::resolve_media_smooth_me_budget_conn]: this file's px², else another row with the same stored decode size, else global.

use rusqlite::Connection;

use super::{resolve_media_smooth_me_budget_conn, DEFAULT_SMOOTH_MAX_AREA};

fn open_schema(c: &Connection) {
    c.execute_batch(
        "CREATE TABLE media (
                path TEXT PRIMARY KEY NOT NULL,
                decode_w INTEGER,
                decode_h INTEGER,
                smooth_me_budget_px2 INTEGER,
                smooth_me_budget_updated_at INTEGER
            );",
    )
    .unwrap();
}

#[test]
fn uses_own_saved_px2_ignores_global_pref() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/a.mkv", 1920, 1080, 800_000_i64],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/a.mkv", g).unwrap(), 800_000);
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/a.mkv", 900_000_u64).unwrap(),
        800_000
    );
}

#[test]
fn same_decode_neighbor_else_global_no_row_uses_global() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/a.mkv", 1920, 1080, 800_000_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/b.mkv", 3840, 2160, 600_000_i64],
    )
    .unwrap();

    let g = DEFAULT_SMOOTH_MAX_AREA;
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/new.mkv", 3840, 2160],
    )
    .unwrap();
    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(), 600_000);

    c.execute(
        "UPDATE media SET decode_w = 1920, decode_h = 1080 WHERE path = '/new.mkv'",
        (),
    )
    .unwrap();
    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(), 800_000);

    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/unknown.mkv", g).unwrap(), g);
}

#[test]
fn neighbor_tie_prefers_latest_updated_at_then_rowid() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2, smooth_me_budget_updated_at)
             VALUES (?, ?, ?, ?, ?)",
        rusqlite::params!["/lo.mkv", 1920, 1080, 900_000_i64, 100_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2, smooth_me_budget_updated_at)
             VALUES (?, ?, ?, ?, ?)",
        rusqlite::params!["/hi.mkv", 1920, 1080, 1_200_000_i64, 5_000_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/q.mkv", 1920, 1080],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/q.mkv", g).unwrap(), 1_200_000);

    let c2 = Connection::open_in_memory().unwrap();
    open_schema(&c2);
    c2.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/older.mkv", 1920, 1080, 800_000_i64],
    )
    .unwrap();
    c2.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/newer.mkv", 1920, 1080, 1_267_644_i64],
    )
    .unwrap();
    c2.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/q2.mkv", 1920, 1080],
    )
    .unwrap();
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c2, "/q2.mkv", g).unwrap(),
        1_267_644
    );
}

#[test]
fn neighbor_can_exceed_global_pref() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/learned.mkv", 1920, 1080, 1_200_000_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/other.mkv", 1920, 1080],
    )
    .unwrap();
    let g = 900_000_u64;
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/other.mkv", g).unwrap(),
        1_200_000
    );
}

#[test]
fn no_other_with_budget_at_same_decode_falls_back_global() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/dims_only.mkv", 1280, 720],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/new.mkv", 1920, 1080],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    assert_eq!(resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(), g);
}
