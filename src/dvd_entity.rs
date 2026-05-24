//! DVD title structure (chapter lists, timeline helpers). Persistence keys live in
//! [`crate::playback_entity`] — one row per title, not per chapter `.vob`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dvd_vob_timeline::DvdVobTimeline;

/// `VTS_02_3` → `3` (VOB part within the title set). Used by `dvd-ifo` timeline build and tests.
pub(crate) fn vob_part_id(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?.to_ascii_uppercase();
    let rest = stem.strip_prefix("VTS_")?;
    let mut parts = rest.split('_');
    let _vts = parts.next()?;
    parts.next()?.parse().ok()
}

/// `VTS_02_1` → `2`.
pub(crate) fn vob_title_id(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?.to_ascii_uppercase();
    let rest = stem.strip_prefix("VTS_")?;
    rest.split('_').next()?.parse().ok()
}

fn is_playable_chapter_vob(path: &Path) -> bool {
    crate::video_ext::is_dvd_vob_path(path) && vob_part_id(path).is_some_and(|n| n >= 1)
}

/// `VIDEO_TS/` for `current`, preferring the disc-root child over `current.parent()` casing.
pub(crate) fn video_ts_for_vob(current: &Path) -> Option<PathBuf> {
    if let Some(disc) = crate::video_ext::dvd_disc_root(current) {
        if let Some(vts) = crate::video_ext::dvd_video_ts_dir(&disc) {
            return Some(vts);
        }
    }
    let parent = current.parent()?;
    parent.is_dir().then(|| parent.to_path_buf())
}

/// All feature chapter `.vob` files on the disc (`VTS_02_1` … `VTS_03_N`, …). Skips menu
/// `VTS_01_*` when any `VTS_02+` set exists (same rule as main-title pick).
pub(crate) fn list_feature_vobs(current: &Path) -> Vec<PathBuf> {
    let Some(parent) = current.parent() else {
        return Vec::new();
    };
    let vts = video_ts_for_vob(current).unwrap_or_else(|| parent.to_path_buf());
    let Ok(read) = std::fs::read_dir(&vts) else {
        return Vec::new();
    };
    let playable: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_playable_chapter_vob(p))
        .collect();
    let skip_menu = playable
        .iter()
        .filter_map(|p| vob_title_id(p))
        .any(|t| t >= 2);
    let mut v: Vec<PathBuf> = playable
        .into_iter()
        .filter(|p| {
            let Some(tid) = vob_title_id(p) else {
                return false;
            };
            !skip_menu || tid >= 2
        })
        .collect();
    v.sort_by(|a, b| {
        lexical_sort::natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    v
}

/// Chapter `.vob` files for one `VTS_XX` title set only (helpers/tests).
pub(crate) fn list_title_vobs(_vts: &Path, current: &Path) -> Vec<PathBuf> {
    let Some(title) = vob_title_id(current) else {
        return Vec::new();
    };
    list_feature_vobs(current)
        .into_iter()
        .filter(|p| vob_title_id(p) == Some(title))
        .collect()
}

/// First on-disk chapter `.vob` for a title set (`part` 1 if present).
pub(crate) fn first_chapter_vob(vts: &Path, title_id: u32) -> Option<PathBuf> {
    let vts_dir = video_ts_for_vob(vts)?;
    let Ok(read) = std::fs::read_dir(&vts_dir) else {
        return None;
    };
    let mut v: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_playable_chapter_vob(p))
        .filter(|p| vob_title_id(p) == Some(title_id))
        .collect();
    v.sort_by(|a, b| {
        lexical_sort::natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    v.into_iter().next()
}

/// All chapter paths for the same title as `path`.
pub(crate) fn title_chapter_paths(path: &Path) -> Option<Vec<PathBuf>> {
    if !crate::video_ext::is_dvd_vob_path(path) {
        return None;
    }
    let vobs = list_feature_vobs(path);
    (!vobs.is_empty()).then_some(vobs)
}

/// First chapter `.vob` used to build a title timeline from a disc folder or chapter path.
pub(crate) fn timeline_chapter_probe(path: &Path) -> Option<PathBuf> {
    if crate::video_ext::is_dvd_vob_path(path) {
        return Some(path.to_path_buf());
    }
    let disc = crate::video_ext::dvd_disc_root(path)?;
    crate::video_ext::dvd_main_chapter_vob(&disc)
}

/// Disc root containing `VIDEO_TS/` (SQLite / history key) and chapter list for one title.
pub(crate) fn title_playback_entity(path: &Path) -> Option<(PathBuf, Vec<PathBuf>)> {
    let chapter_probe = if crate::video_ext::is_dvd_vob_path(path) {
        path.to_path_buf()
    } else if let Some(disc) = crate::video_ext::dvd_disc_root(path) {
        crate::video_ext::dvd_main_chapter_vob(&disc)?
    } else {
        return None;
    };
    let chapters = title_chapter_paths(&chapter_probe)?;
    let disc = crate::video_ext::dvd_disc_root(&chapter_probe)?;
    let db_key = std::fs::canonicalize(&disc).ok().unwrap_or(disc);
    Some((db_key, chapters))
}

/// First chapter `.vob` in the title set (legacy SQLite key before disc-root entity).
pub(crate) fn title_entity_path(path: &Path) -> Option<PathBuf> {
    title_chapter_paths(path)?
        .into_iter()
        .next()
        .and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)))
}

/// Drop legacy per-chapter `.vob` `media` rows after consolidating onto the title entity.
pub(crate) fn purge_chapter_media_rows(entity: &Path) {
    let Some((disc_key, chapters)) = title_playback_entity(entity).or_else(|| {
        let chapters = title_chapter_paths(entity)?;
        Some((crate::playback_entity::db_path_for(entity), chapters))
    }) else {
        return;
    };
    for ch in &chapters {
        if crate::video_ext::paths_same_file(ch, &disc_key) {
            continue;
        }
        crate::db::delete_media_row_exact(ch);
        if let Ok(c) = std::fs::canonicalize(ch) {
            crate::db::delete_media_row_exact(&c);
        }
    }
    if let Some(vob) = crate::video_ext::dvd_first_playable_vob(&disc_key) {
        if let Some(legacy) = title_entity_path(&vob) {
            if !crate::video_ext::paths_same_file(&legacy, &disc_key) {
                crate::db::delete_media_row_exact(&legacy);
                if let Ok(c) = std::fs::canonicalize(&legacy) {
                    crate::db::delete_media_row_exact(&c);
                }
            }
        }
    }
}

include!("dvd_entity_timeline.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn entity_path_is_first_chapter_in_title() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-ent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        for n in ["VTS_02_1.VOB", "VTS_02_2.VOB"] {
            fs::write(vts.join(n), b"v").expect("write");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let (disc_key, _) = title_playback_entity(&p1).expect("entity");
        assert!(crate::video_ext::paths_same_file(&disc_key, &base));
        assert_eq!(
            title_entity_path(&p1).as_deref(),
            title_entity_path(&p2).as_deref()
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resume_maps_past_first_vob_with_per_file_durs() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-res-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        let sizes = [100usize, 200, 300, 400];
        for (i, n) in sizes.iter().enumerate() {
            fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; *n]).expect("vob");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        let mut durs = HashMap::new();
        durs.insert(p1.to_string_lossy().into_owned(), 100.0);
        durs.insert(
            vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
            200.0,
        );
        durs.insert(p3.to_string_lossy().into_owned(), 300.0);
        let (load, local) = resume_chapter_and_local(&p1, 350.0, &durs).expect("target");
        assert!(crate::video_ext::paths_same_file(&load, &p3));
        assert!((local - 50.0).abs() < 1e-6, "local={local}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn still_target_past_first_vob_with_per_file_durs() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        let sizes = [100usize, 200, 300, 400];
        for (i, n) in sizes.iter().enumerate() {
            fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; *n]).expect("vob");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        let mut durs = HashMap::new();
        durs.insert(p1.to_string_lossy().into_owned(), 100.0);
        durs.insert(
            vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
            200.0,
        );
        durs.insert(p3.to_string_lossy().into_owned(), 300.0);
        let still = still_target_from_global(&p1, 350.0, &durs).expect("still");
        assert!(crate::video_ext::paths_same_file(&still.load, &p3));
        assert!((still.local_sec - 50.0).abs() < 1e-6, "local={}", still.local_sec);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn purge_targets_chapter_vob_rows_not_entity() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-purge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"v").expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), b"v").expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let entity = crate::playback_entity::db_path_for(&p1);
        let exact = crate::db::media_path_key_exact(&p1).expect("exact");
        let entity_k = crate::db::history_key(&entity).expect("entity");
        assert_ne!(exact, entity_k);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn title_playback_entity_uses_disc_root() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-disc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"v").expect("write");
        let (key, chapters) = title_playback_entity(&base).expect("disc entity");
        assert!(crate::video_ext::paths_same_file(&key, &base));
        assert_eq!(chapters.len(), 1);
        let _ = fs::remove_dir_all(&base);
    }
}
