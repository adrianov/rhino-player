//! Tests smooth ME budget resolution (`media_me_budget.rs`).

use rusqlite::Connection;

use super::{resolve_media_smooth_me_budget_conn, DEFAULT_SMOOTH_MAX_AREA};

#[test]
fn own_row_uses_global_else_exact_dimension_neighbor() {
    let c = Connection::open_in_memory().unwrap();
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
    let r = resolve_media_smooth_me_budget_conn(&c, "/a.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, DEFAULT_SMOOTH_MAX_AREA);

    let r = resolve_media_smooth_me_budget_conn(
        &c,
        "/a.mkv",
        Some((1920, 1080)),
        800_000_u64,
    )
    .unwrap();
    assert_eq!(r, 800_000);

    let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((3840, 2160)), g).unwrap();
    assert_eq!(r, 600_000);

    let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, 800_000);

    let r = resolve_media_smooth_me_budget_conn(&c, "/solo.mkv", Some((640, 480)), g).unwrap();
    assert_eq!(r, g);
}

#[test]
fn exact_dims_tie_prefers_latest_updated_at_over_rowid() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2, smooth_me_budget_updated_at)
             VALUES (?, ?, ?, ?, ?)",
        rusqlite::params!["/a.ms_hi.mkv", 1920, 1080, 1_200_000_i64, 5_000_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2, smooth_me_budget_updated_at)
             VALUES (?, ?, ?, ?, ?)",
        rusqlite::params!["/b.ms_lo.mkv", 1920, 1080, 900_000_i64, 100_i64],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    let r = resolve_media_smooth_me_budget_conn(&c, "/brand.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, 1_200_000);
}

#[test]
fn exact_dims_tie_uses_rowid_when_updated_at_equal() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/older.mkv", 1920, 1080, 800_000_i64],
    )
    .unwrap();
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/newer.mkv", 1920, 1080, 1_267_644_i64],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    let r = resolve_media_smooth_me_budget_conn(&c, "/brand.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, 1_267_644);
}

#[test]
fn neighbor_above_global_still_applies() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/learned.mkv", 1920, 1080, 1_200_000_i64],
    )
    .unwrap();
    let g = 900_000_u64;
    let r = resolve_media_smooth_me_budget_conn(&c, "/other.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, 1_200_000);
}

#[test]
fn own_row_never_below_global_after_recovery_or_prefs() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/stale.mkv", 1920, 1080, 1_085_325_i64],
    )
    .unwrap();
    let r = resolve_media_smooth_me_budget_conn(
        &c,
        "/stale.mkv",
        Some((1920, 1080)),
        1_193_858_u64,
    )
    .unwrap();
    assert_eq!(r, 1_193_858);
}

#[test]
fn own_high_row_follows_lower_global_after_overload() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h, smooth_me_budget_px2) VALUES (?, ?, ?, ?)",
        rusqlite::params!["/heavy.mkv", 1920, 1080, 543_036_i64],
    )
    .unwrap();
    let r = resolve_media_smooth_me_budget_conn(
        &c,
        "/heavy.mkv",
        Some((1920, 1080)),
        488_732_u64,
    )
    .unwrap();
    assert_eq!(r, 488_732);
}

#[test]
fn global_when_no_other_row_has_saved_budget() {
    let c = Connection::open_in_memory().unwrap();
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
    c.execute(
        "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
        rusqlite::params!["/dims_only.mkv", 1280, 720],
    )
    .unwrap();
    let g = DEFAULT_SMOOTH_MAX_AREA;
    let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((1920, 1080)), g).unwrap();
    assert_eq!(r, g);
}
