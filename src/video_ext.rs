//! Video filename extensions: Open dialog, sibling **Prev/Next**, and folder scanning share one list.
//! Optical-disc layouts: Blu-ray **BDMV** ([bluray_disc_root]) and DVD **VIDEO_TS** ([dvd_disc_root]).

use std::path::{Path, PathBuf};

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
    let direct = disc.join("VIDEO_TS");
    if direct.is_dir() && video_ts_has_ifo(&direct) {
        return Some(direct);
    }
    find_video_ts_child(disc).filter(|vts| video_ts_has_ifo(vts))
}

/// Same local media path (canonical when possible; no DVD “same disc” merge).
pub(crate) fn paths_same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Next `.vob` in the same `VIDEO_TS/` (EOF / **Next** on folder-less mpv DVD playback).
pub fn next_dvd_vob(current: &Path) -> Option<PathBuf> {
    dvd_vob_sibling(current, 1)
}

/// Previous `.vob` in the same `VIDEO_TS/`.
pub fn prev_dvd_vob(current: &Path) -> Option<PathBuf> {
    dvd_vob_sibling(current, -1)
}

fn dvd_vob_sibling(current: &Path, step: isize) -> Option<PathBuf> {
    let vts = current.parent().filter(|p| is_video_ts_dir_name(p))?;
    let vobs = list_vobs_in_video_ts(vts);
    let i = vobs
        .iter()
        .position(|p| paths_same_file(p, current))?
        as isize;
    let j = i + step;
    (j >= 0 && (j as usize) < vobs.len()).then(|| vobs[j as usize].clone())
}

/// First chapter VOB in `VIDEO_TS/` for mpv builds without `dvd://` (often no libdvdread).
pub fn dvd_first_playable_vob(disc: &Path) -> Option<PathBuf> {
    let vts = dvd_video_ts_dir(disc)?;
    pick_first_dvd_vob(&vts)
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

fn list_vobs_in_video_ts(vts: &Path) -> Vec<PathBuf> {
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

fn vob_stem_upper(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_uppercase())
}

fn pick_first_dvd_vob(vts: &Path) -> Option<PathBuf> {
    let vobs = list_vobs_in_video_ts(vts);
    if vobs.is_empty() {
        return None;
    }
    for p in &vobs {
        let Some(stem) = vob_stem_upper(p) else {
            continue;
        };
        if stem.starts_with("VTS_") && stem.ends_with("_1") {
            return Some(p.clone());
        }
    }
    let content: Vec<&PathBuf> = vobs
        .iter()
        .filter(|p| vob_stem_upper(p).as_deref() != Some("VIDEO_TS"))
        .filter(|p| vob_stem_upper(p).is_none_or(|s| !s.ends_with("_0")))
        .collect();
    let pool: Vec<&PathBuf> = if content.is_empty() { vobs.iter().collect() } else { content };
    pool.iter()
        .max_by_key(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
        .map(|p| (*p).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn bluray_root_from_disc_and_bdmv_package() {
        let base = std::env::temp_dir().join(format!("rhino-bluray-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let disc = base.join("Disc");
        let bdmv = disc.join("BDMV");
        fs::create_dir_all(&bdmv).expect("mkdir");
        fs::write(bdmv.join("MovieObject.bdmv"), b"MOBJ0200").expect("write");
        assert_eq!(bluray_disc_root(&disc).as_deref(), Some(disc.as_path()));
        assert_eq!(bluray_disc_root(&bdmv).as_deref(), Some(disc.as_path()));
        assert_eq!(
            bluray_disc_root(&bdmv.join("MovieObject.bdmv")).as_deref(),
            Some(disc.as_path())
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_root_from_disc_and_video_ts_folder() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let disc = base.join("DVD1");
        let vts = disc.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVDVIDEO").expect("write");
        assert_eq!(dvd_disc_root(&disc).as_deref(), Some(disc.as_path()));
        assert_eq!(dvd_disc_root(&vts).as_deref(), Some(disc.as_path()));
        let mixed = base.join("Mgnoveniy");
        let vts2 = mixed.join("Video_ts");
        fs::create_dir_all(&vts2).expect("mkdir");
        fs::write(vts2.join("VIDEO_TS.IFO"), b"IFO").expect("write");
        assert_eq!(dvd_disc_root(&mixed).as_deref(), Some(mixed.as_path()));
        assert_eq!(dvd_disc_root(&vts2).as_deref(), Some(mixed.as_path()));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_resolve_opens_first_chapter_vob() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-vob-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let disc = base.join("DVD1");
        let vts = disc.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"IFO").expect("write");
        fs::write(vts.join("VIDEO_TS.VOB"), vec![0u8; 64]).expect("write");
        fs::write(vts.join("VTS_01_0.VOB"), vec![0u8; 128]).expect("write");
        fs::write(vts.join("VTS_01_1.VOB"), vec![0u8; 4096]).expect("write");
        fs::write(vts.join("VTS_01_2.VOB"), vec![0u8; 2048]).expect("write");
        assert_eq!(
            resolve_open_media_path(&disc),
            vts.join("VTS_01_1.VOB")
        );
        assert_eq!(
            dvd_first_playable_vob(&disc).as_deref(),
            Some(vts.join("VTS_01_1.VOB").as_path())
        );
        assert_eq!(
            next_dvd_vob(&vts.join("VTS_01_1.VOB")).as_deref(),
            Some(vts.join("VTS_01_2.VOB").as_path())
        );
        assert!(next_dvd_vob(&vts.join("VTS_01_2.VOB")).is_none());
        let ch2 = vts.join("VTS_01_2.VOB");
        assert_eq!(resolve_open_media_path(&disc), vts.join("VTS_01_1.VOB"));
        assert_eq!(resolve_open_media_path(&ch2), ch2);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_vob_path_and_broadcast_fps() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-fps-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("Video_ts");
        fs::create_dir_all(&vts).expect("mkdir");
        let ch = vts.join("VTS_02_1.VOB");
        fs::write(&ch, b"x").expect("write");
        assert!(is_dvd_vob_path(&ch));
        assert!(!is_dvd_vob_path(&base.join("clip.mkv")));
        assert_eq!(dvd_vob_broadcast_fps(Some((768, 576))), Some(25.0));
        assert_eq!(dvd_vob_broadcast_fps(Some((720, 576))), Some(25.0));
        assert!((dvd_vob_broadcast_fps(Some((720, 480))).unwrap() - 30000.0 / 1001.0).abs() < 1e-6);
        assert!(dvd_vob_broadcast_fps(Some((1280, 720))).is_none());
        let _ = fs::remove_dir_all(&base);
    }
}
