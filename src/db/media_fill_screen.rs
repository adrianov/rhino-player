// Per-file Fill Screen choice on `media` rows: applied whenever that file plays in fullscreen.
// `false` is meaningful too — the user explicitly chose the fitted view for this video.

/// Stored Fill Screen choice for [path] (`None` = never toggled; global fitted default applies).
#[must_use]
pub(crate) fn media_fill_screen(path: &std::path::Path) -> Option<bool> {
    let key = history_key(path)?;
    with_conn(|c| stored_fill_screen(c, &key)).flatten()
}

fn stored_fill_screen(c: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<bool>> {
    Ok(c.query_row(
        "SELECT fill_screen FROM media WHERE path = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()?
    .flatten())
}

/// Persist the user's explicit Fill toggle for [path] (written only from the button handler).
pub(crate) fn media_save_fill_screen(path: &std::path::Path, on: bool) {
    let Some(key) = history_key(path) else {
        return;
    };
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, fill_screen) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET fill_screen = excluded.fill_screen",
            params![&key, on],
        )?;
        Ok(())
    });
}
