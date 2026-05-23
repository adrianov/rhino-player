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

/// Chapter `.vob` files for the same DVD title as `current` (not the whole `VIDEO_TS/` tree).
pub(crate) fn list_title_vobs(vts: &Path, current: &Path) -> Vec<PathBuf> {
    let Some(title) = vob_title_id(current) else {
        return Vec::new();
    };
    let vts = video_ts_for_vob(current).unwrap_or_else(|| vts.to_path_buf());
    let Ok(read) = std::fs::read_dir(&vts) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_playable_chapter_vob(p))
        .filter(|p| vob_title_id(p) == Some(title))
        .collect();
    v.sort_by(|a, b| {
        lexical_sort::natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    v
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

/// Chapter path for `title` / `part` when the file exists under `vts`.
pub(crate) fn chapter_vob_if_exists(vts: &Path, title: u32, part: u32) -> Option<PathBuf> {
    let probe = vts.join(format!("VTS_{title:02}_1.VOB"));
    let vts = video_ts_for_vob(&probe)?;
    list_title_vobs(&vts, &probe)
        .into_iter()
        .find(|p| vob_part_id(p) == Some(part))
}

/// All chapter paths for the same title as `path`.
pub(crate) fn title_chapter_paths(path: &Path) -> Option<Vec<PathBuf>> {
    if !crate::video_ext::is_dvd_vob_path(path) {
        return None;
    }
    let vts = path.parent()?;
    let vobs = list_title_vobs(vts, path);
    (!vobs.is_empty()).then_some(vobs)
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

/// Drop per-chapter `media` rows after consolidating onto the title entity.
pub(crate) fn purge_chapter_media_rows(entity: &Path) {
    let Some((disc_key, chapters)) = title_playback_entity(entity).or_else(|| {
        let chapters = title_chapter_paths(entity)?;
        Some((crate::playback_entity::db_path_for(entity), chapters))
    }) else {
        return;
    };
    let entity_s = crate::db::history_key(&disc_key);
    for ch in &chapters {
        if crate::video_ext::paths_same_file(ch, &disc_key) {
            continue;
        }
        crate::db::delete_media_row_exact(ch);
        if let Ok(c) = std::fs::canonicalize(ch) {
            if entity_s.as_deref() != c.to_str() {
                crate::db::delete_media_row_exact(&c);
            }
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

/// Global title time and total duration for persistence (seconds).
pub(crate) fn playback_snapshot(
    chapter: &Path,
    local_pos: f64,
    local_dur: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    if !crate::video_ext::is_dvd_vob_path(chapter) {
        return None;
    }
    let entity = crate::playback_entity::db_path_for(chapter);
    let entity_key = entity.to_string_lossy();
    let mut tl = DvdVobTimeline::from_chapter_ifo(chapter)
        .or_else(|| DvdVobTimeline::from_chapter(chapter, dur_by_path, chapter, local_dur))?;
    if let Some(on_disk) = title_chapter_paths(chapter) {
        tl.expand_on_disk_chapters(&on_disk);
    }
    if let Some(total) = dur_by_path.get(entity_key.as_ref()).copied() {
        if total.is_finite() && total > tl.total_sec {
            tl.apply_entity_total(total);
        }
    }
    let global = tl.global_pos(chapter, local_pos);
    let total = tl.total_sec.max(local_dur);
    Some((total, global))
}

/// Map stored global resume to `(chapter path, local offset)` for `loadfile`.
pub(crate) fn resume_chapter_and_local(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(PathBuf, f64)> {
    let mut tl = DvdVobTimeline::from_chapter_ifo(chapter)
        .or_else(|| DvdVobTimeline::from_chapter_db_only(chapter, dur_by_path))?;
    let entity = crate::playback_entity::db_path_for(chapter);
    let key = entity.to_string_lossy();
    if let Some(total) = dur_by_path.get(key.as_ref()).copied() {
        if total.is_finite() && total > 0.0 {
            tl.apply_entity_total(total);
        }
    }
    if tl.total_sec <= 0.0 {
        return None;
    }
    let (idx, local) = tl.resolve_global(global_sec);
    let target = tl.path_at(idx)?.to_path_buf();
    Some((target, local))
}

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
