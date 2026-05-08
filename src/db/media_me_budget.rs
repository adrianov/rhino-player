// Per-file smooth ME pixel budget + decode dimensions on `media` rows (bundled script).

/// Persist decode width/height for [path] (canonical key). Upserts without touching other columns.
pub(crate) fn media_sync_decode_size(path: &std::path::Path, w: i32, h: i32) {
    if w <= 0 || h <= 0 {
        return;
    }
    let Some(key) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, decode_w, decode_h) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
               decode_w = excluded.decode_w,
               decode_h = excluded.decode_h",
            params![&key, w, h],
        )?;
        Ok(())
    });
}

/// Save per-file ME budget px² after adaptive overload (or future explicit per-file edits).
pub(crate) fn media_save_smooth_me_budget(path: &std::path::Path, px: u64) {
    let Some(key) = history_key(path) else {
        return;
    };
    let px = px.max(MIN_SMOOTH_MAX_AREA).min(i64::MAX as u64) as i64;
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, smooth_me_budget_px2) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET
               smooth_me_budget_px2 = excluded.smooth_me_budget_px2",
            params![&key, px],
        )?;
        Ok(())
    });
}

/// Effective ME budget: this file's stored px² if set, else closest other file with dims+budget, else [global_px].
#[must_use]
pub(crate) fn resolve_media_smooth_me_budget(
    path: Option<&std::path::Path>,
    decode_wh: Option<(i32, i32)>,
    global_px: u64,
) -> u64 {
    let global = global_px.max(MIN_SMOOTH_MAX_AREA);
    let Some(path) = path else {
        return global;
    };
    let Some(key) = history_key(path) else {
        return global;
    };
    with_conn(|c| resolve_media_smooth_me_budget_conn(c, &key, decode_wh, global)).unwrap_or(global)
}

fn resolve_media_smooth_me_budget_conn(
    c: &rusqlite::Connection,
    path_key: &str,
    decode_wh: Option<(i32, i32)>,
    global: u64,
) -> rusqlite::Result<u64> {
    let own = c
        .query_row(
            "SELECT smooth_me_budget_px2 FROM media WHERE path = ?1",
            params![path_key],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .filter(|&px| px > 0)
        .map(|px| px as u64);
    if let Some(px) = own {
        return Ok(px.max(MIN_SMOOTH_MAX_AREA));
    }

    let (dw, dh) = match decode_wh {
        Some((w, h)) if w > 0 && h > 0 => (w, h),
        _ => return Ok(global),
    };

    let mut stmt = c.prepare(
        "SELECT smooth_me_budget_px2, decode_w, decode_h FROM media
         WHERE path != ?1
           AND smooth_me_budget_px2 IS NOT NULL AND smooth_me_budget_px2 > 0
           AND decode_w IS NOT NULL AND decode_h IS NOT NULL
           AND decode_w > 0 AND decode_h > 0",
    )?;
    let mut best: Option<(i64, u64)> = None;
    let mut rows = stmt.query(params![path_key])?;
    while let Some(row) = rows.next()? {
        let px2: i64 = row.get(0)?;
        let ow: i64 = row.get(1)?;
        let oh: i64 = row.get(2)?;
        if px2 <= 0 {
            continue;
        }
        let d = (ow - i64::from(dw)).pow(2) + (oh - i64::from(dh)).pow(2);
        best = match best {
            None => Some((d, px2 as u64)),
            Some((bd, _)) if d < bd => Some((d, px2 as u64)),
            Some(b) => Some(b),
        };
    }
    Ok(best
        .map(|(_, px)| px)
        .unwrap_or(global)
        .max(MIN_SMOOTH_MAX_AREA))
}

#[cfg(test)]
mod media_me_budget_tests {
    use super::resolve_media_smooth_me_budget_conn;
    use rusqlite::Connection;

    #[test]
    fn own_row_wins_then_neighbor_then_global() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE media (
                path TEXT PRIMARY KEY NOT NULL,
                decode_w INTEGER,
                decode_h INTEGER,
                smooth_me_budget_px2 INTEGER
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

        let g = 2_000_000_u64;
        let r = resolve_media_smooth_me_budget_conn(&c, "/a.mkv", Some((1920, 1080)), g).unwrap();
        assert_eq!(r, 800_000);

        let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((3840, 2160)), g).unwrap();
        assert_eq!(r, 600_000);

        let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((1920, 1080)), g).unwrap();
        assert_eq!(r, 800_000);

        let r = resolve_media_smooth_me_budget_conn(&c, "/solo.mkv", Some((640, 480)), g).unwrap();
        assert_eq!(r, 800_000);
    }

    #[test]
    fn global_when_no_other_row_has_saved_budget() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE media (
                path TEXT PRIMARY KEY NOT NULL,
                decode_w INTEGER,
                decode_h INTEGER,
                smooth_me_budget_px2 INTEGER
            );",
        )
        .unwrap();
        c.execute(
            "INSERT INTO media (path, decode_w, decode_h) VALUES (?, ?, ?)",
            rusqlite::params!["/dims_only.mkv", 1280, 720],
        )
        .unwrap();
        let g = 2_000_000_u64;
        let r = resolve_media_smooth_me_budget_conn(&c, "/new.mkv", Some((1920, 1080)), g).unwrap();
        assert_eq!(r, g);
    }
}
