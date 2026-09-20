// Per-file Fill Screen prefs on `media` rows: user fill toggle + strip-probe cache.

use crate::black_bars::CropRect;

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
    if mtime_ns == stamp.mtime_ns && size == stamp.size {
        return decode_bar_crop(&spec);
    }
    None
}

/// Wire format in `media.bar_crop`: `c` = probed clean; `d:c` = clean, Bob was in vf;
/// `WxH+X+Y` / `d:WxH+X+Y` = crop (`d:` = Bob seen). Legacy `""` / `d` rows predate
/// probed-clean caching — decoded as unprobed (`None`) so the next open re-probes.
fn decode_bar_crop(spec: &str) -> Option<StoredBarCrop> {
    let (rest, saw_deint) = match spec.strip_prefix("d:") {
        Some(rest) => (rest, true),
        None => (spec, false),
    };
    if rest.is_empty() {
        return None; // legacy "" / "d": written before "no strips" meant a real probe ran
    }
    let crop = match rest {
        "c" => None,
        _ => Some(CropRect::parse_video_crop(rest)?.as_video_crop()),
    };
    Some(StoredBarCrop { crop, saw_deint })
}

fn encode_bar_crop(crop: Option<&str>, saw_deint: bool) -> String {
    match (crop, saw_deint) {
        (None, false) => "c".into(),
        (None, true) => "d:c".into(),
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
    fn bar_crop_round_trips_probed_rows() {
        for (crop, deint) in [
            (None, false),
            (None, true),
            (Some("1920x800+0+140"), false),
            (Some("1920x800+0+140"), true),
        ] {
            let enc = encode_bar_crop(crop, deint);
            assert_eq!(
                decode_bar_crop(&enc),
                Some(StoredBarCrop {
                    crop: crop.map(str::to_string),
                    saw_deint: deint,
                })
            );
        }
    }

    #[test]
    fn legacy_and_corrupt_rows_decode_as_unprobed() {
        assert_eq!(decode_bar_crop(""), None);
        assert_eq!(decode_bar_crop("d"), None);
        assert_eq!(decode_bar_crop("garbage"), None);
        // Crop rows from before the marker stayed valid (real metadata was required).
        assert_eq!(
            decode_bar_crop("1920x800+0+140"),
            Some(StoredBarCrop {
                crop: Some("1920x800+0+140".into()),
                saw_deint: false,
            })
        );
        assert_eq!(
            decode_bar_crop("d:1920x800+0+140"),
            Some(StoredBarCrop {
                crop: Some("1920x800+0+140".into()),
                saw_deint: true,
            })
        );
    }

    #[test]
    fn fresh_bar_crop_requires_matching_ns_mtime_and_size() {
        let stamp = FileStamp {
            mtime_ns: 1_700_000_000_123_456_789,
            size: 42,
        };
        let row = |spec: &str, mtime: i64, size: i64| {
            Some((Some(spec.to_string()), Some(mtime), Some(size)))
        };
        let hit = fresh_bar_crop(row("c", stamp.mtime_ns, stamp.size), stamp);
        assert_eq!(
            hit,
            Some(StoredBarCrop {
                crop: None,
                saw_deint: false
            })
        );
        // Legacy "" cache is unprobed even with a matching stamp.
        assert!(fresh_bar_crop(row("", stamp.mtime_ns, stamp.size), stamp).is_none());
        assert!(fresh_bar_crop(row("c", stamp.mtime_ns + 1, stamp.size), stamp).is_none());
        assert!(fresh_bar_crop(row("c", stamp.mtime_ns, stamp.size + 1), stamp).is_none());
        // Legacy second-only rows (no ns/size) miss.
        assert!(fresh_bar_crop(Some((Some("c".to_string()), None, None)), stamp).is_none());
    }
}
