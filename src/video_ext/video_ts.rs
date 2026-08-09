//! On-disk DVD `VIDEO_TS` directory: IFO presence, chapter VOBs, broadcast FPS.

use std::path::{Path, PathBuf};

/// `VIDEO_TS/` folder that holds the menu IFO and title-set chapter VOBs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTsDir {
    dir: PathBuf,
}

impl VideoTsDir {
    /// Resolve `VIDEO_TS/` under a DVD disc root (case-insensitive `Video_ts`, etc.).
    pub fn under_disc(disc: &Path) -> Option<Self> {
        let direct = disc.join("VIDEO_TS");
        if Self::is_dir_name(&direct) && Self::has_menu_ifo(&direct) {
            return Some(Self { dir: direct });
        }
        Self::find_child(disc)
    }

    /// `path` itself is a `VIDEO_TS` directory with a menu IFO.
    pub fn at(path: &Path) -> Option<Self> {
        if Self::is_dir_name(path) && Self::has_menu_ifo(path) {
            Some(Self {
                dir: path.to_path_buf(),
            })
        } else {
            None
        }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn list_vobs(&self) -> Vec<PathBuf> {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
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

    pub fn title_set_bytes(&self, title_id: u32) -> u64 {
        crate::dvd_entity::chapter_vobs_for_title_pub(&self.dir, title_id)
            .iter()
            .filter_map(|p| p.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    fn find_child(parent: &Path) -> Option<Self> {
        let Ok(entries) = std::fs::read_dir(parent) else {
            return None;
        };
        for e in entries.flatten() {
            let p = e.path();
            if let Some(vts) = Self::at(&p) {
                return Some(vts);
            }
        }
        None
    }

    fn is_dir_name(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO_TS"))
    }

    fn has_menu_ifo(dir: &Path) -> bool {
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
}

pub(super) fn is_vob_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("vob"))
}

pub(super) fn is_playable_dvd_chapter(path: &Path) -> bool {
    crate::dvd_entity::vob_part_id(path).is_some_and(|n| n >= 1)
}

/// Chapter `.vob` under `VIDEO_TS/` (folder-open DVD playback without `dvd://`).
pub fn is_dvd_vob_path(path: &Path) -> bool {
    is_vob_file(path) && path.parent().is_some_and(VideoTsDir::is_dir_name)
}

/// Broadcast cadence when the engine omits container FPS on ripped DVD chapters
/// (PAL 576-line vs NTSC 480-line).
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
    VideoTsDir {
        dir: vts.to_path_buf(),
    }
    .list_vobs()
}

pub(super) fn title_set_bytes(vts_dir: &Path, title_id: u32) -> u64 {
    VideoTsDir {
        dir: vts_dir.to_path_buf(),
    }
    .title_set_bytes(title_id)
}
