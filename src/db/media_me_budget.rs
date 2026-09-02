// Per-file smooth ME pixel budget + decode dimensions on `media` rows (bundled script).

/// Whether [media_sync_decode_size] should write **`decode_w/h`**: never shrink stored pixel area.
#[must_use]
pub(super) fn media_decode_size_update_allowed(prior: Option<(i32, i32)>, w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    let new_a = (w as i64) * (h as i64);
    match prior {
        Some((ow, oh)) if ow > 0 && oh > 0 => new_a >= (ow as i64) * (oh as i64),
        _ => true,
    }
}

/// Persist decode width/height for [path] (canonical key). Upserts without touching other columns.
///
/// Skips updates that **shrink** the stored pixel area: after **`vf vapoursynth`** attaches, mpv’s
/// **`video-params`** / **`width`×`height`** can briefly reflect **scaled vf output** while the
/// bundled `.vpy` still uses full-frame decode. Shrinking **`decode_w`×`decode_h`** would change
/// [resolve_media_smooth_me_budget] and force a redundant **`vf clr`/`vf add`**.
pub(crate) fn media_sync_decode_size(path: &std::path::Path, w: i32, h: i32) {
    if w <= 0 || h <= 0 {
        return;
    }
    let Some(key) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        let prior = stored_decode_dims(c, &key)?;
        if !media_decode_size_update_allowed(prior, w, h) {
            return Ok(());
        }
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

/// Stored decode width/height for [path] (canonical key). Used by continue-card quality tags.
pub fn media_decode_size(path: &std::path::Path) -> Option<(i32, i32)> {
    let key = history_key(path)?;
    with_conn(|c| stored_decode_dims(c, &key)).flatten()
}

/// Stored positive `decode_w`×`decode_h` for one path key (`None` when absent or not positive).
fn stored_decode_dims(c: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<(i32, i32)>> {
    Ok(c.query_row(
        "SELECT decode_w, decode_h FROM media WHERE path = ?1",
        params![key],
        |row| {
            let w: Option<i32> = row.get(0)?;
            let h: Option<i32> = row.get(1)?;
            Ok(match (w, h) {
                (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                _ => None,
            })
        },
    )
    .optional()?
    .flatten())
}

/// This row's own saved **`smooth_me_budget_px2`** when set to a positive value.
fn own_saved_px2(c: &rusqlite::Connection, path_key: &str) -> rusqlite::Result<Option<u64>> {
    let px: Option<i64> = c
        .query_row(
            "SELECT smooth_me_budget_px2 FROM media WHERE path = ?1",
            params![path_key],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    Ok(px.filter(|&px| px > 0).map(|px| px as u64))
}

/// Latest same-decode-size neighbor budget (latest `updated_at`, then `rowid`); `None` when no such row.
fn neighbor_saved_px2(
    c: &rusqlite::Connection,
    path_key: &str,
    dw: i32,
    dh: i32,
) -> rusqlite::Result<Option<u64>> {
    let px: Option<i64> = c
        .query_row(
            "SELECT smooth_me_budget_px2 FROM media
             WHERE path != ?1
               AND decode_w = ?2 AND decode_h = ?3
               AND smooth_me_budget_px2 IS NOT NULL AND smooth_me_budget_px2 > 0
             ORDER BY COALESCE(smooth_me_budget_updated_at, 0) DESC, rowid DESC
             LIMIT 1",
            params![path_key, dw, dh],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    Ok(px.map(|px| px as u64))
}

#[cfg(test)]
mod media_decode_size_update_tests {
    use super::media_decode_size_update_allowed;

    #[test]
    fn rejects_smaller_decode_area() {
        assert!(!media_decode_size_update_allowed(
            Some((1920, 1080)),
            1696,
            952
        ));
    }

    #[test]
    fn allows_fresh_insert_and_growth() {
        assert!(media_decode_size_update_allowed(None, 1696, 952));
        assert!(media_decode_size_update_allowed(Some((100, 100)), 200, 200));
        assert!(media_decode_size_update_allowed(
            Some((1920, 1080)),
            1920,
            1080
        ));
    }
}

/// Unix millis ([`std::time::UNIX_EPOCH`]) for [media_save_smooth_me_budget] (tie-break among exact-dimension rows).
fn smooth_me_budget_updated_at_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// Save per-file ME budget px² after adaptive overload (or future explicit per-file edits).
pub(crate) fn media_save_smooth_me_budget(path: &std::path::Path, px: u64) {
    let Some(key) = history_key(path) else {
        return;
    };
    let px = px.max(MIN_SMOOTH_MAX_AREA).min(i64::MAX as u64) as i64;
    let updated_at = smooth_me_budget_updated_at_now();
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, smooth_me_budget_px2, smooth_me_budget_updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
               smooth_me_budget_px2 = excluded.smooth_me_budget_px2,
               smooth_me_budget_updated_at = excluded.smooth_me_budget_updated_at",
            params![&key, px, updated_at],
        )?;
        Ok(())
    });
}

/// Effective ME px²: this row's **`smooth_me_budget_px2`** if set; else **`smooth_me_budget_px2`**
/// from another row with the same stored **`decode_w`**×**`decode_h`** (latest **`smooth_me_budget_updated_at`**, then **`rowid`**);
/// else **`global_px`**. Neighbor runs only after this file's decode size exists in **`media`** (avoids stale mpv size after a switch).
#[must_use]
pub(crate) fn resolve_media_smooth_me_budget(
    path: Option<&std::path::Path>,
    global_px: u64,
) -> u64 {
    let global = global_px.max(MIN_SMOOTH_MAX_AREA);
    let Some(path) = path else {
        return global;
    };
    let Some(key) = history_key(path) else {
        return global;
    };
    with_conn(|c| resolve_media_smooth_me_budget_conn(c, &key, global)).unwrap_or(global)
}

pub(super) fn resolve_media_smooth_me_budget_conn(
    c: &rusqlite::Connection,
    path_key: &str,
    global: u64,
) -> rusqlite::Result<u64> {
    if let Some(px) = own_saved_px2(c, path_key)? {
        return Ok(px.max(MIN_SMOOTH_MAX_AREA));
    }
    let Some((dw, dh)) = stored_decode_dims(c, path_key)? else {
        return Ok(global);
    };
    let px = neighbor_saved_px2(c, path_key, dw, dh)?.unwrap_or(global);
    Ok(px.max(MIN_SMOOTH_MAX_AREA))
}
