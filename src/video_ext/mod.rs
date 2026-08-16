//! Video filename extensions and optical-disc open paths.
//! Open dialog, sibling **Prev/Next**, and folder scanning share [SUFFIX].

use std::path::{Path, PathBuf};

mod dvd_pick;
mod folder_scan;
mod optical_disc;
mod video_ts;

pub(crate) use folder_scan::list_videos_in_dir;
use folder_scan::{dir_has_videos, folder_open_entry};

pub use optical_disc::{
    bluray_disc_root, dvd_disc_root, dvd_first_playable_vob, dvd_main_chapter_vob,
    dvd_video_ts_dir, is_bluray_disc_path, is_dvd_disc_path, is_optical_disc_path, OpticalDisc,
};
pub use video_ts::{dvd_vob_broadcast_fps, is_dvd_vob_path};

pub(crate) fn feature_title_set_id(vts: &Path) -> Option<u32> {
    dvd_pick::feature_title_set_id(vts)
}

pub(crate) fn resolve_dvd_main_vts(vts_dir: &Path, srpt_vts: u32, bytes_vts: u32) -> u32 {
    dvd_pick::resolve_main_title_id(vts_dir, Some(srpt_vts), bytes_vts)
}

/// Lowercase extensions (no leading dot) for “is this a video file?” in a directory.
/// Kept in sync with the **Open Video** filter; extend here only.
/// **`ts`**: MPEG transport stream → `video/mp2t` on the desktop entry.
/// **`mpg` / `mpeg` / `vob`**: MPEG program stream → `video/mpeg` + macOS `public.mpeg` /
/// `jp.co.dvdfllc.vob`.
/// **`dctmp`**: in-progress Direct Connect download (often `name.mkv.<id>.dctmp`) →
/// `application/x-dcpp-incomplete` in `data/mime/packages/` + desktop / AppStream / Info.plist.
pub const SUFFIX: &[&str] = &[
    "3g2", "3gp", "asf", "avi", "dctmp", "divx", "dvr-ms", "f4v", "flv", "h264", "h265", "hevc",
    "m2ts", "m4v", "mkv", "mov", "mpeg", "mpg", "mp4", "mts", "mxf", "nsv", "ogv", "rmp4", "ts",
    "vob", "webm", "wmv", "xvid", "y4m", "yuv",
];

/// `true` for a regular file whose extension is in [SUFFIX] (case-insensitive).
pub fn is_video_path(p: &Path) -> bool {
    p.is_file()
        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            let l = e.to_ascii_lowercase();
            SUFFIX.contains(&l.as_str())
        })
}

/// Local path acceptable for **Open**, CLI boot, and external `open` handlers.
pub fn is_openable_media_path(path: &Path) -> bool {
    is_video_path(path) || is_optical_disc_path(path) || dir_has_videos(path)
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

/// Normalize paths before load: Blu-ray → disc root; DVD **folder** → first chapter `.vob`;
/// an ordinary directory → last in-progress file (natural sort) or the first video;
/// an existing `.vob` file is kept (sibling advance must not rewind to `VTS_01_1`).
pub fn resolve_open_media_path(path: &Path) -> PathBuf {
    match OpticalDisc::detect(path) {
        Some(disc @ OpticalDisc::BluRay { .. }) => disc.root().to_path_buf(),
        Some(dvd @ OpticalDisc::Dvd { .. }) if !video_ts::is_vob_file(path) => dvd.open_target(),
        Some(OpticalDisc::Dvd { .. }) => path.to_path_buf(),
        None if path.is_dir() => folder_open_entry(path).unwrap_or_else(|| {
            eprintln!("[rhino] open: no playable files in {}", path.display());
            path.to_path_buf()
        }),
        None => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests;
