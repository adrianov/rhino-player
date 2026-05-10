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

/// Effective ME budget: **`global_px`** (prefs **`video_smooth_max_area`**, clamped) alone when this path's **`media`** row stores **`smooth_me_budget_px2`** (prefs stay authoritative; per-file column mirrors adaptive persist). Otherwise **`smooth_me_budget_px2`** from **another** row whose **`decode_w`**×**`decode_h`** **exactly** matches the current decode size; ties use **`smooth_me_budget_updated_at`** then **`rowid`**. The chosen row may be **below or above** **`global_px`**. **`global_px`** applies when decode size is unknown or **no** other row shares those dimensions with a saved budget.
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

pub(super) fn resolve_media_smooth_me_budget_conn(
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
    if own.is_some() {
        // Prefs row drives bundled ME cap for known media: overload lowers **`video_smooth_max_area`**
        // while **`smooth_me_budget_px2`** can lag one stale write or duplicate keys — never keep **`max(own, global)`**
        // or a **high** DB column blocks shrink (recovery raises already lift **`global`** above stale lows).
        return Ok(global.max(MIN_SMOOTH_MAX_AREA));
    }

    let (dw, dh) = match decode_wh {
        Some((w, h)) if w > 0 && h > 0 => (w, h),
        _ => return Ok(global),
    };

    let neighbor_px = c
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

    Ok(neighbor_px
        .map(|px| px as u64)
        .unwrap_or(global)
        .max(MIN_SMOOTH_MAX_AREA))
}
