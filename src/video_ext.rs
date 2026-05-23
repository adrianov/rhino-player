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
    dvd_video_ts_dir_inner(disc)
}

fn dvd_video_ts_dir_inner(disc: &Path) -> Option<PathBuf> {
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

fn is_playable_dvd_chapter(path: &Path) -> bool {
    crate::dvd_entity::vob_part_id(path).is_some_and(|n| n >= 1)
}

/// Main feature: `VIDEO_TS.IFO` title table when present, else `.vob` heuristics.
fn pick_main_dvd_vob(vts: &Path) -> Option<PathBuf> {
    if let Some(disc) = dvd_disc_root(vts) {
        if let Some((vts_id, _ttn)) = crate::dvd_ifo_parse::main_title_from_disc(&disc) {
            let vts_dir = dvd_video_ts_dir(&disc)?;
            if let Some(p) = crate::dvd_entity::first_chapter_vob(&vts_dir, vts_id) {
                return Some(p);
            }
        }
    }
    pick_main_dvd_vob_from_files(vts)
}

/// Fallback when IFO is unavailable: most chapter files, ties → lowest `VTS_XX`.
fn pick_main_dvd_vob_from_files(vts: &Path) -> Option<PathBuf> {
    use std::collections::HashMap;
    let vobs: Vec<PathBuf> = list_vobs_in_video_ts(vts)
        .into_iter()
        .filter(|p| is_playable_dvd_chapter(p))
        .collect();
    if vobs.is_empty() {
        return None;
    }
    let mut by_title: HashMap<u32, (usize, u64, PathBuf)> = HashMap::new();
    for p in &vobs {
        let Some(tid) = crate::dvd_entity::vob_title_id(p) else {
            continue;
        };
        let e = by_title.entry(tid).or_insert_with(|| (0, 0, p.clone()));
        e.0 += 1;
        e.1 += p.metadata().ok().map(|m| m.len() as u64).unwrap_or(0);
        if crate::dvd_entity::vob_part_id(p) == Some(1) {
            e.2 = p.clone();
        }
    }
    let skip_menu = by_title.keys().any(|&t| t >= 2);
    let titles: Vec<(u32, usize, u64, PathBuf)> = by_title
        .into_iter()
        .filter(|(t, _)| !skip_menu || *t != 1)
        .map(|(t, (c, b, p))| (t, c, b, p))
        .collect();
    if titles.is_empty() {
        return None;
    }
    let max_ch = titles.iter().map(|(_, c, _, _)| *c).max()?;
    titles
        .into_iter()
        .filter(|(_, c, _, _)| *c == max_ch)
        .min_by_key(|(t, _, _, _)| *t)
        .map(|(_, _, _, path)| path)
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
    fn pick_main_prefers_lower_title_when_chapter_counts_tie() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-tie-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"IFO").expect("write");
        fs::write(vts.join("VTS_02_4.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_03_1.VOB"), vec![0u8; 500_000]).expect("write");
        assert_eq!(
            pick_main_dvd_vob(&vts).as_deref(),
            Some(vts.join("VTS_02_4.VOB").as_path())
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_resolve_opens_main_title_first_chapter() {
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
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 50_000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 50_000]).expect("write");
        assert_eq!(
            resolve_open_media_path(&disc),
            vts.join("VTS_02_1.VOB")
        );
        assert_eq!(
            dvd_first_playable_vob(&disc).as_deref(),
            Some(vts.join("VTS_02_1.VOB").as_path())
        );
        let title_vobs =
            crate::dvd_entity::list_title_vobs(&vts, &vts.join("VTS_02_1.VOB"));
        assert_eq!(title_vobs.len(), 2);
        assert_eq!(title_vobs[1], vts.join("VTS_02_2.VOB"));
        let ch2 = vts.join("VTS_01_2.VOB");
        assert_eq!(resolve_open_media_path(&disc), vts.join("VTS_02_1.VOB"));
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
