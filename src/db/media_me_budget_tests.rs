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

/// Insert a row with decode size, saved budget, and optional `updated_at`.
fn insert_row(c: &Connection, path: &str, w: i32, h: i32, px: Option<i64>, at: Option<i64>) {
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2, smooth_me_budget_updated_at)
             VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![path, w, h, px, at],
    )
    .unwrap();
}

fn insert_dims_only(c: &Connection, path: &str, w: i32, h: i32) {
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params![path, w, h],
    )
    .unwrap();
}

#[test]
fn uses_own_saved_px2_ignores_global_pref() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    insert_row(&c, "/a.mkv", 1920, 1080, Some(800_000), None);
    let g = DEFAULT_SMOOTH_MAX_AREA;
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/a.mkv", g).unwrap(),
        800_000
    );
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/a.mkv", 900_000_u64).unwrap(),
        800_000
    );
}
#[test]
fn same_decode_neighbor_else_global_no_row_uses_global() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    insert_row(&c, "/a.mkv", 1920, 1080, Some(800_000), None);
    insert_row(&c, "/b.mkv", 3840, 2160, Some(600_000), None);

    let g = DEFAULT_SMOOTH_MAX_AREA;
    insert_dims_only(&c, "/new.mkv", 3840, 2160);
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(),
        600_000
    );

    c.execute(
        "UPDATE media SET decode_w = 1920, decode_h = 1080 WHERE path = '/new.mkv'",
        (),
    )
    .unwrap();
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(),
        800_000
    );

    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/unknown.mkv", g).unwrap(),
        g
    );
}

#[test]
fn neighbor_tie_prefers_latest_updated_at() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    insert_row(&c, "/lo.mkv", 1920, 1080, Some(900_000), Some(100));
    insert_row(&c, "/hi.mkv", 1920, 1080, Some(1_200_000), Some(5_000));
    insert_dims_only(&c, "/q.mkv", 1920, 1080);
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/q.mkv", DEFAULT_SMOOTH_MAX_AREA).unwrap(),
        1_200_000
    );
}

#[test]
fn neighbor_tie_prefers_later_rowid_when_updated_at_equal() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    insert_row(&c, "/older.mkv", 1920, 1080, Some(800_000), None);
    insert_row(&c, "/newer.mkv", 1920, 1080, Some(1_267_644), None);
    insert_dims_only(&c, "/q2.mkv", 1920, 1080);
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/q2.mkv", DEFAULT_SMOOTH_MAX_AREA).unwrap(),
        1_267_644
    );
}

#[test]
fn neighbor_can_exceed_global_pref() {
    let c = Connection::open_in_memory().unwrap();
    open_schema(&c);
    insert_row(&c, "/learned.mkv", 1920, 1080, Some(1_200_000), None);
    insert_dims_only(&c, "/other.mkv", 1920, 1080);
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
    insert_dims_only(&c, "/dims_only.mkv", 1280, 720);
    insert_dims_only(&c, "/new.mkv", 1920, 1080);
    let g = DEFAULT_SMOOTH_MAX_AREA;
    assert_eq!(
        resolve_media_smooth_me_budget_conn(&c, "/new.mkv", g).unwrap(),
        g
    );
}
