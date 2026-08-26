// Adopt a finished download path in continue `history` + `media` primary keys.

/// Move continue rows from [from] (exact stored path) to [to].
/// On conflict, keep [to] and prefer [from]'s `media` row (resume from the incomplete file).
/// Returns false if the DB is unavailable or [from] has no history row.
pub fn rekey_continue_path(from: &Path, to: &Path) -> bool {
    let Some(from_s) = from.to_str().map(str::to_owned) else {
        return false;
    };
    let Some(to_s) = history_key(to) else {
        return false;
    };
    if from_s == to_s {
        return true;
    }
    with_conn(|c| {
        if !rekey_history_conn(c, &from_s, &to_s)? {
            return Ok(false);
        }
        rekey_media_conn(c, &from_s, &to_s)?;
        Ok(true)
    })
    .unwrap_or(false)
}

/// Returns whether a history row for [from] was updated or removed.
fn rekey_history_conn(c: &Connection, from: &str, to: &str) -> rusqlite::Result<bool> {
    let Some(from_opened) = history_last_opened(c, from)? else {
        return Ok(false);
    };
    let to_exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM history WHERE path = ?1)",
        params![to],
        |row| row.get(0),
    )?;
    if to_exists {
        merge_history_into_target(c, from, to, from_opened)?;
    } else {
        c.execute(
            "UPDATE history SET path = ?1 WHERE path = ?2",
            params![to, from],
        )?;
    }
    Ok(true)
}

/// `last_opened` of one history row (`None` when the row is absent).
fn history_last_opened(c: &Connection, path: &str) -> rusqlite::Result<Option<i64>> {
    c.query_row(
        "SELECT last_opened FROM history WHERE path = ?1",
        params![path],
        |row| row.get(0),
    )
    .optional()
}

/// Target exists: keep its row (max `last_opened`) and drop [from]'s.
fn merge_history_into_target(
    c: &Connection,
    from: &str,
    to: &str,
    from_opened: i64,
) -> rusqlite::Result<()> {
    c.execute(
        "UPDATE history SET last_opened = MAX(last_opened, ?1) WHERE path = ?2",
        params![from_opened, to],
    )?;
    c.execute("DELETE FROM history WHERE path = ?1", params![from])?;
    Ok(())
}

fn rekey_media_conn(c: &Connection, from: &str, to: &str) -> rusqlite::Result<()> {
    if !media_path_exists(c, from)? {
        return Ok(());
    }
    if media_path_exists(c, to)? {
        c.execute("DELETE FROM media WHERE path = ?1", params![to])?;
    }
    c.execute(
        "UPDATE media SET path = ?1 WHERE path = ?2",
        params![to, from],
    )?;
    let _ = c.execute(
        "UPDATE media SET thumb_load_path = ?1 WHERE thumb_load_path = ?2",
        params![to, from],
    );
    Ok(())
}

/// Whether a `media` row exists for this exact path string.
fn media_path_exists(c: &Connection, path: &str) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM media WHERE path = ?1)",
        params![path],
        |row| row.get(0),
    )
}
