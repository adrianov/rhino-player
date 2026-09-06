// Per-file Fill Screen prefs on `media` rows: user fill toggle + strip-probe cache.

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

/// Cached lavfi strip probe: `crop` is `None` when clean; `saw_deint` matches Bob-in-vf at probe time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredBarCrop {
    pub crop: Option<String>,
    pub saw_deint: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileStamp {
    mtime_ns: i64,
    size: i64,
}

fn file_stamp(path: &std::path::Path) -> Option<FileStamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(FileStamp {
        mtime_ns: mtime_ns(&meta)?,
        size: i64::try_from(meta.len()).ok()?,
    })
}

fn mtime_ns(meta: &std::fs::Metadata) -> Option<i64> {
    i64::try_from(
        meta.modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
    .ok()
}

/// Fresh strip cache for [path] when mtime (ns) and size still match the file on disk.
#[must_use]
pub(crate) fn media_bar_crop(path: &std::path::Path) -> Option<StoredBarCrop> {
    let key = history_key(path)?;
    let stamp = file_stamp(path)?;
    with_conn(|c| stored_bar_crop(c, &key, stamp)).flatten()
}

fn stored_bar_crop(
    c: &rusqlite::Connection,
    key: &str,
    stamp: FileStamp,
) -> rusqlite::Result<Option<StoredBarCrop>> {
    let row = c
        .query_row(
            "SELECT bar_crop, bar_crop_mtime_ns, bar_crop_size FROM media WHERE path = ?1",
            params![key],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(fresh_bar_crop(row, stamp))
}

fn fresh_bar_crop(
    row: Option<(Option<String>, Option<i64>, Option<i64>)>,
    stamp: FileStamp,
) -> Option<StoredBarCrop> {
    let (Some(spec), Some(mtime_ns), Some(size)) = row? else {
        return None;
    };
    (mtime_ns == stamp.mtime_ns && size == stamp.size).then(|| decode_bar_crop(&spec))
}

/// Wire format in `media.bar_crop`: `""` / `d` = clean; `WxH+X+Y` / `d:WxH+X+Y` = crop (`d` = Bob seen).
fn decode_bar_crop(spec: &str) -> StoredBarCrop {
    if spec.is_empty() {
        return StoredBarCrop {
            crop: None,
            saw_deint: false,
        };
    }
    if spec == "d" {
        return StoredBarCrop {
            crop: None,
            saw_deint: true,
        };
    }
    if let Some(rest) = spec.strip_prefix("d:") {
        return StoredBarCrop {
            crop: (!rest.is_empty()).then(|| rest.to_string()),
            saw_deint: true,
        };
    }
    StoredBarCrop {
        crop: Some(spec.to_string()),
        saw_deint: false,
    }
}

fn encode_bar_crop(crop: Option<&str>, saw_deint: bool) -> String {
    match (crop, saw_deint) {
        (None, false) => String::new(),
        (None, true) => "d".into(),
        (Some(s), false) => s.into(),
        (Some(s), true) => format!("d:{s}"),
    }
}

/// Persist a finished strip probe (`None` crop = clean / no strips).
pub(crate) fn media_save_bar_crop(path: &std::path::Path, crop: Option<&str>, saw_deint: bool) {
    let Some(key) = history_key(path) else {
        return;
    };
    let Some(stamp) = file_stamp(path) else {
        eprintln!("[rhino] bars: skip crop cache save (no stamp) path={key}");
        return;
    };
    let spec = encode_bar_crop(crop, saw_deint);
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO media (path, bar_crop, bar_crop_mtime_ns, bar_crop_size)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               bar_crop = excluded.bar_crop,
               bar_crop_mtime_ns = excluded.bar_crop_mtime_ns,
               bar_crop_size = excluded.bar_crop_size",
            params![&key, spec, stamp.mtime_ns, stamp.size],
        )?;
        Ok(())
    });
}

#[cfg(test)]
mod bar_crop_codec_tests {
    use super::{decode_bar_crop, encode_bar_crop, fresh_bar_crop, FileStamp, StoredBarCrop};

    #[test]
    fn bar_crop_round_trips_deint_flag() {
        for (crop, deint) in [
            (None, false),
            (None, true),
            (Some("1920x800+0+140"), false),
            (Some("1920x800+0+140"), true),
        ] {
            let enc = encode_bar_crop(crop, deint);
            assert_eq!(
                decode_bar_crop(&enc),
                StoredBarCrop {
                    crop: crop.map(str::to_string),
                    saw_deint: deint,
                }
            );
        }
    }

    #[test]
    fn fresh_bar_crop_requires_matching_ns_mtime_and_size() {
        let stamp = FileStamp {
            mtime_ns: 1_700_000_000_123_456_789,
            size: 42,
        };
        let hit = fresh_bar_crop(
            Some((Some(String::new()), Some(stamp.mtime_ns), Some(stamp.size))),
            stamp,
        );
        assert_eq!(
            hit,
            Some(StoredBarCrop {
                crop: None,
                saw_deint: false
            })
        );
        assert!(fresh_bar_crop(
            Some((Some(String::new()), Some(stamp.mtime_ns + 1), Some(stamp.size))),
            stamp,
        )
        .is_none());
        assert!(fresh_bar_crop(
            Some((Some(String::new()), Some(stamp.mtime_ns), Some(stamp.size + 1))),
            stamp,
        )
        .is_none());
        // Legacy second-only rows (no ns/size) miss.
        assert!(fresh_bar_crop(Some((Some(String::new()), None, None)), stamp).is_none());
    }
}
