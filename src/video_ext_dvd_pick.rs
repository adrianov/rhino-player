// Main-feature `.vob` pick: IFO title + on-disk bytes, skipping stub `VTS_xx_1` chapters.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{
    dvd_disc_root, dvd_video_ts_dir, is_playable_dvd_chapter, list_vobs_in_video_ts,
    title_set_bytes,
};

/// Main feature: `VIDEO_TS.IFO` when sane, else largest title set on disk.
pub(super) fn pick_main_dvd_vob(vts: &Path) -> Option<PathBuf> {
    let bytes_pick = pick_main_dvd_vob_from_files(vts);
    let Some(disc) = dvd_disc_root(vts) else {
        return chapter_entry_for_title(vts, bytes_pick);
    };
    let Some(vts_dir) = dvd_video_ts_dir(&disc) else {
        return chapter_entry_for_title(vts, bytes_pick);
    };
    let ifo_tid = crate::dvd_ifo_parse::main_title_from_disc(&disc);
    let Some(bytes_pick) = bytes_pick else {
        let (tid, _ttn) = ifo_tid?;
        return crate::dvd_entity::first_chapter_vob(&vts_dir, tid);
    };
    let bytes_tid = crate::dvd_entity::vob_title_id(&bytes_pick)?;
    let (title_id, _ttn) = ifo_tid.unwrap_or((bytes_tid, 1));
    let title_id = resolve_main_title_id(&vts_dir, Some(title_id), bytes_tid);
    crate::dvd_entity::first_chapter_vob(&vts_dir, title_id)
        .or((title_id == bytes_tid).then_some(bytes_pick))
}

pub(super) fn chapter_entry_for_title(vts: &Path, part_one: Option<PathBuf>) -> Option<PathBuf> {
    let part_one = part_one?;
    let tid = crate::dvd_entity::vob_title_id(&part_one)?;
    crate::dvd_entity::first_chapter_vob(vts, tid)
        .or(Some(part_one))
}

pub(super) fn resolve_main_title_id(vts_dir: &Path, ifo_tid: Option<u32>, bytes_tid: u32) -> u32 {
    let Some(ifo_tid) = ifo_tid else {
        return bytes_tid;
    };
    if ifo_tid == bytes_tid {
        return ifo_tid;
    }
    let ifo_bytes = title_set_bytes(vts_dir, ifo_tid);
    let main_bytes = title_set_bytes(vts_dir, bytes_tid);
    if main_bytes > ifo_bytes.saturating_mul(4) {
        bytes_tid
    } else {
        ifo_tid
    }
}

/// Fallback when IFO is unavailable: largest title set by on-disk bytes; ties → lowest `VTS_XX`.
pub(super) fn feature_title_set_id(vts: &Path) -> Option<u32> {
    pick_main_dvd_vob_from_files(vts).and_then(|p| crate::dvd_entity::vob_title_id(&p))
}

fn pick_main_dvd_vob_from_files(vts: &Path) -> Option<PathBuf> {
    let vobs: Vec<PathBuf> = list_vobs_in_video_ts(vts)
        .into_iter()
        .filter(|p| is_playable_dvd_chapter(p))
        .collect();
    if vobs.is_empty() {
        return None;
    }
    let mut by_title: HashMap<u32, (u64, PathBuf)> = HashMap::new();
    for p in &vobs {
        let Some(tid) = crate::dvd_entity::vob_title_id(p) else {
            continue;
        };
        let e = by_title.entry(tid).or_insert_with(|| (0, p.clone()));
        e.0 += p.metadata().ok().map(|m| m.len()).unwrap_or(0);
        if crate::dvd_entity::vob_part_id(p) == Some(1) {
            e.1 = p.clone();
        }
    }
    const MENU_TITLE_BYTES: u64 = 100_000_000;
    let skip_vts1_menu = by_title.get(&1).is_some_and(|(b, _)| *b < MENU_TITLE_BYTES)
        && by_title.keys().any(|&t| t >= 2);
    let titles: Vec<(u32, u64, PathBuf)> = by_title
        .into_iter()
        .filter(|(t, _)| !skip_vts1_menu || *t != 1)
        .map(|(t, (b, p))| (t, b, p))
        .collect();
    if titles.is_empty() {
        return None;
    }
    titles
        .into_iter()
        .max_by(|a, b| match a.1.cmp(&b.1) {
            std::cmp::Ordering::Equal => b.0.cmp(&a.0),
            other => other,
        })
        .map(|(_, _, path)| path)
}
