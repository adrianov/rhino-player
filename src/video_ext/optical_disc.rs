//! Blu-ray / AVCHD and DVD disc trees: layout detection and open targets.

use std::path::{Path, PathBuf};

use super::dvd_pick::pick_main_dvd_vob;
use super::video_ts::VideoTsDir;

const MOVIE_OBJECT_NAMES: &[&str] = &["MovieObject.bdmv", "MOVIEOBJ.BDM"];

/// Recognized optical disc rooted at the folder that contains `BDMV/` or `VIDEO_TS/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpticalDisc {
    BluRay { root: PathBuf },
    Dvd { root: PathBuf },
}

impl OpticalDisc {
    /// Detect a Blu-ray or DVD layout from a path under that disc.
    pub fn detect(path: &Path) -> Option<Self> {
        if let Some(root) = Self::bluray_root(path) {
            return Some(Self::BluRay { root });
        }
        if let Some(root) = Self::dvd_root(path) {
            return Some(Self::Dvd { root });
        }
        None
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::BluRay { root } | Self::Dvd { root } => root,
        }
    }

    /// `VIDEO_TS/` for DVD; `None` for Blu-ray.
    pub fn video_ts_dir(&self) -> Option<VideoTsDir> {
        match self {
            Self::Dvd { root } => VideoTsDir::under_disc(root),
            Self::BluRay { .. } => None,
        }
    }

    /// Engine open path: Blu-ray disc root, or DVD chapter (resume-aware).
    /// Callers that already have a chapter `.vob` should keep that path instead.
    pub fn open_target(&self) -> PathBuf {
        match self {
            Self::BluRay { root } => root.clone(),
            Self::Dvd { root } => first_playable_vob(root).unwrap_or_else(|| root.clone()),
        }
    }

    fn bluray_root(path: &Path) -> Option<PathBuf> {
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

    fn dvd_root(path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            // Inside VIDEO_TS only — a neighbouring transport folder must not capture other videos.
            let vts = VideoTsDir::at(path.parent()?)?;
            return vts.path().parent().map(Path::to_path_buf);
        }
        if let Some(vts) = VideoTsDir::at(path) {
            return vts.path().parent().map(Path::to_path_buf);
        }
        VideoTsDir::under_disc(path).map(|_| path.to_path_buf())
    }
}

/// Disc root for a Blu-ray / AVCHD **BDMV** tree (parent of `BDMV/` when applicable).
pub fn bluray_disc_root(path: &Path) -> Option<PathBuf> {
    match OpticalDisc::detect(path) {
        Some(disc @ OpticalDisc::BluRay { .. }) => Some(disc.root().to_path_buf()),
        _ => None,
    }
}

/// Disc root for a DVD **VIDEO_TS** tree (directory that contains `VIDEO_TS/` with `VIDEO_TS.IFO`).
pub fn dvd_disc_root(path: &Path) -> Option<PathBuf> {
    OpticalDisc::dvd_root(path)
}

/// `true` when `path` is a disc root, `BDMV/` package dir, or `MovieObject.bdmv`.
pub fn is_bluray_disc_path(path: &Path) -> bool {
    matches!(OpticalDisc::detect(path), Some(OpticalDisc::BluRay { .. }))
}

/// `true` when `path` is a DVD root or `VIDEO_TS/` with a menu IFO.
pub fn is_dvd_disc_path(path: &Path) -> bool {
    OpticalDisc::dvd_root(path).is_some()
}

/// Blu-ray or DVD folder tree.
pub fn is_optical_disc_path(path: &Path) -> bool {
    OpticalDisc::detect(path).is_some()
}

/// `VIDEO_TS/` under a DVD disc root (case-insensitive `Video_ts`, etc.).
pub fn dvd_video_ts_dir(disc: &Path) -> Option<PathBuf> {
    OpticalDisc::Dvd {
        root: disc.to_path_buf(),
    }
    .video_ts_dir()
    .map(|v| v.path().to_path_buf())
}

/// Main-feature first chapter for entity / timeline probe (no resume redirect).
pub fn dvd_main_chapter_vob(disc: &Path) -> Option<PathBuf> {
    pick_main_dvd_vob(&dvd_video_ts_dir(disc)?)
}

/// Chapter to load when opening a DVD folder (resume may pick a later chapter).
pub fn dvd_first_playable_vob(disc: &Path) -> Option<PathBuf> {
    first_playable_vob(disc)
}

fn first_playable_vob(disc: &Path) -> Option<PathBuf> {
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

fn movie_object_in(dir: &Path) -> bool {
    MOVIE_OBJECT_NAMES
        .iter()
        .any(|name| dir.join(name).is_file())
}
