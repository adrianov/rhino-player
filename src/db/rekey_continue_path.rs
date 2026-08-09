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
    let from_opened: Option<i64> = c
        .query_row(
            "SELECT last_opened FROM history WHERE path = ?1",
            params![from],
            |row| row.get(0),
        )
        .optional()?;
    let Some(from_opened) = from_opened else {
        return Ok(false);
    };
    let to_exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM history WHERE path = ?1)",
        params![to],
        |row| row.get(0),
    )?;
    if to_exists {
        c.execute(
            "UPDATE history SET last_opened = MAX(last_opened, ?1) WHERE path = ?2",
            params![from_opened, to],
        )?;
        c.execute("DELETE FROM history WHERE path = ?1", params![from])?;
    } else {
        c.execute(
            "UPDATE history SET path = ?1 WHERE path = ?2",
            params![to, from],
        )?;
    }
    Ok(true)
}

fn rekey_media_conn(c: &Connection, from: &str, to: &str) -> rusqlite::Result<()> {
    let from_exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM media WHERE path = ?1)",
        params![from],
        |row| row.get(0),
    )?;
    if !from_exists {
        return Ok(());
    }
    let to_exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM media WHERE path = ?1)",
        params![to],
        |row| row.get(0),
    )?;
    if to_exists {
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
