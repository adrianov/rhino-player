//! Video filename extensions: Open dialog, sibling **Prev/Next**, and folder scanning share one list.
//! Optical-disc layouts: Blu-ray **BDMV** ([bluray_disc_root]) and DVD **VIDEO_TS** ([dvd_disc_root]).

use std::path::{Path, PathBuf};

mod dvd_pick {
    include!("video_ext_dvd_pick.rs");
}
use dvd_pick::pick_main_dvd_vob;

pub(crate) fn feature_title_set_id(vts: &Path) -> Option<u32> {
    dvd_pick::feature_title_set_id(vts)
}

pub(crate) fn resolve_dvd_main_vts(vts_dir: &Path, srpt_vts: u32, bytes_vts: u32) -> u32 {
    dvd_pick::resolve_main_title_id(vts_dir, Some(srpt_vts), bytes_vts)
}

/// Lowercase extensions (no leading dot) for “is this a video file?” in a directory.
/// Kept in sync with the **Open Video** file filter; extend here only.
/// **`ts`**: MPEG transport stream; pair with `video/mp2t` in `data/applications/*.desktop` for “Open with”.
pub const SUFFIX: &[&str] = &[
    "3g2", "3gp", "asf", "avi", "divx", "dvr-ms", "f4v", "flv", "h264", "h265", "hevc", "m2ts",
    "m4v", "mkv", "mov", "mpeg", "mpg", "mp4", "mts", "mxf", "nsv", "ogv", "rmp4", "ts", "vob",
    "webm", "wmv", "xvid", "y4m", "yuv",
];

/// `true` for a regular file whose extension is in [SUFFIX] (case-insensitive).
pub fn is_video_path(p: &Path) -> bool {
    p.is_file()
        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            let l = e.to_ascii_lowercase();
            SUFFIX.contains(&l.as_str())
        })
}

const MOVIE_OBJECT_NAMES: &[&str] = &["MovieObject.bdmv", "MOVIEOBJ.BDM"];

fn movie_object_in(dir: &Path) -> bool {
    MOVIE_OBJECT_NAMES
        .iter()
        .any(|name| dir.join(name).is_file())
}

fn is_video_ts_dir_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO_TS"))
}

fn video_ts_has_ifo(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO_TS.IFO"))
    })
}

fn find_video_ts_child(parent: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return None;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && is_video_ts_dir_name(&p) {
            return Some(p);
        }
    }
    None
}

/// Disc root for a Blu-ray / AVCHD **BDMV** tree (parent of `BDMV/` when applicable).
pub fn bluray_disc_root(path: &Path) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if path.is_file() {
        path.parent().map(|p| vec![p.to_path_buf()])?
    } else {
        let mut v = vec![path.to_path_buf()];
        let bdmv = path.join("BDMV");
        if bdmv.is_dir() {
            v.push(bdmv);
        }
        v
    };
    for root in candidates {
        if !movie_object_in(&root) {
            continue;
        }
        let disc = if root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("BDMV"))
        {
            root.parent()?.to_path_buf()
        } else {
            root
        };
        return Some(disc);
    }
    None
}

/// Disc root for a DVD **VIDEO_TS** tree (directory that contains `VIDEO_TS/` with `VIDEO_TS.IFO`).
pub fn dvd_disc_root(path: &Path) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if path.is_file() {
        path.parent().map(|p| vec![p.to_path_buf()])?
    } else {
        vec![path.to_path_buf()]
    };
    for root in candidates {
        if is_video_ts_dir_name(&root) && video_ts_has_ifo(&root) {
            return root.parent().map(|p| p.to_path_buf());
        }
        if let Some(vts) = find_video_ts_child(&root) {
            if video_ts_has_ifo(&vts) {
                return Some(root);
            }
        }
    }
    None
}

/// `true` when `path` is a disc root, `BDMV/` package dir, or `MovieObject.bdmv`.
pub fn is_bluray_disc_path(path: &Path) -> bool {
    bluray_disc_root(path).is_some()
}

/// `true` when `path` is a DVD root or `VIDEO_TS/` with a menu IFO.
pub fn is_dvd_disc_path(path: &Path) -> bool {
    dvd_disc_root(path).is_some()
}

/// Blu-ray or DVD folder tree.
pub fn is_optical_disc_path(path: &Path) -> bool {
    is_bluray_disc_path(path) || is_dvd_disc_path(path)
}

/// Local path acceptable for **Open**, CLI boot, and external `open` handlers.
pub fn is_openable_media_path(path: &Path) -> bool {
    is_video_path(path) || is_optical_disc_path(path)
}

/// `VIDEO_TS/` under a DVD disc root (case-insensitive `Video_ts`, etc.).
pub fn dvd_video_ts_dir(disc: &Path) -> Option<PathBuf> {
    dvd_video_ts_dir_inner(disc)
}

fn dvd_video_ts_dir_inner(disc: &Path) -> Option<PathBuf> {
    let direct = disc.join("VIDEO_TS");
    if direct.is_dir() && video_ts_has_ifo(&direct) {
        return Some(direct);
    }
    find_video_ts_child(disc).filter(|vts| video_ts_has_ifo(vts))
}

/// Same local media path (canonical when possible; case-insensitive fallback for exFAT / `Video_ts`).
pub(crate) fn paths_same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(x), Ok(y)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        if x == y {
            return true;
        }
    }
    path_components_eq_ignore_ascii(a, b)
}

fn path_components_eq_ignore_ascii(a: &Path, b: &Path) -> bool {
    use std::path::Component;
    let ac: Vec<_> = a.components().collect();
    let bc: Vec<_> = b.components().collect();
    if ac.len() != bc.len() {
        return false;
    }
    ac.iter().zip(bc.iter()).all(|(ca, cb)| match (ca, cb) {
        (Component::Normal(x), Component::Normal(y)) => x.eq_ignore_ascii_case(y),
        _ => ca == cb,
    })
}

/// Main-feature first chapter for entity / timeline probe (no resume redirect).
pub fn dvd_main_chapter_vob(disc: &Path) -> Option<PathBuf> {
    pick_main_dvd_vob(&dvd_video_ts_dir(disc)?)
}

/// Chapter to `loadfile` when opening a DVD folder (resume may pick a later chapter).
pub fn dvd_first_playable_vob(disc: &Path) -> Option<PathBuf> {
    let vts = dvd_video_ts_dir(disc)?;
    let main = pick_main_dvd_vob(&vts)?;
    let ent = crate::playback_entity::PlaybackEntity::resolve(&main);
    let key = ent.db_path();
    let map = crate::db::load_duration_map();
    if let Some(global) = crate::db::resume_pos(&key) {
        if let Some((target, _)) = ent.resume_load_target(&main, global, &map) {
            return Some(target);
        }
    }
    Some(main)
}

/// Normalize paths before `loadfile`: Blu-ray → disc root; DVD **folder** → first chapter `.vob`;
/// an existing `.vob` file is kept (sibling advance must not rewind to `VTS_01_1`).
pub fn resolve_open_media_path(path: &Path) -> PathBuf {
    if let Some(disc) = bluray_disc_root(path) {
        return disc;
    }
    if path.is_file() && is_vob_file(path) {
        return path.to_path_buf();
    }
    if let Some(disc) = dvd_disc_root(path) {
        return dvd_first_playable_vob(&disc).unwrap_or(disc);
    }
    path.to_path_buf()
}

fn is_vob_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("vob"))
}

/// Chapter `.vob` under `VIDEO_TS/` (folder-open DVD playback without `dvd://`).
pub fn is_dvd_vob_path(path: &Path) -> bool {
    is_vob_file(path) && path.parent().is_some_and(is_video_ts_dir_name)
}

/// Broadcast cadence when mpv omits `container-fps` on ripped DVD chapters (PAL 576-line vs NTSC 480-line).
pub fn dvd_vob_broadcast_fps(decode_wh: Option<(i32, i32)>) -> Option<f64> {
    let (_w, h) = decode_wh?;
    if h == 576 {
        return Some(25.0);
    }
    if (464..=486).contains(&h) {
        return Some(30000.0 / 1001.0);
    }
    None
}

pub(super) fn list_vobs_in_video_ts(vts: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(vts) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_vob_file(p))
        .collect();
    v.sort_by(|a, b| {
        lexical_sort::natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    v
}

pub(super) fn is_playable_dvd_chapter(path: &Path) -> bool {
    crate::dvd_entity::vob_part_id(path).is_some_and(|n| n >= 1)
}

pub(super) fn title_set_bytes(vts_dir: &Path, title_id: u32) -> u64 {
    crate::dvd_entity::chapter_vobs_for_title_pub(vts_dir, title_id)
        .iter()
        .filter_map(|p| p.metadata().ok())
        .map(|m| m.len())
        .sum()
}

#[cfg(test)]
#[path = "video_ext_tests.rs"]
mod tests;
