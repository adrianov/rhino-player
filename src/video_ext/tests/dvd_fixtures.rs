//! Shared DVD / optical-disc fixtures and assertion helpers for the `video_ext` tests.

use super::super::{
    dvd_disc_root, dvd_first_playable_vob, dvd_vob_broadcast_fps, is_dvd_disc_path,
    is_optical_disc_path, resolve_open_media_path,
};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn scratch(label: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("rhino-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    base
}

pub(super) fn put_bytes(path: &Path, contents: &[u8]) {
    fs::write(path, contents).expect("write");
}

/// Fixed-size zero-filled chapter VOB stub.
pub(super) fn put_vob(path: &Path, len: usize) {
    fs::write(path, vec![0u8; len]).expect("write");
}

/// Create `dir` holding a menu IFO; returns `dir`.
pub(super) fn make_video_ts(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("mkdir");
    put_bytes(&dir.join("VIDEO_TS.IFO"), b"DVDVIDEO");
    dir.to_path_buf()
}

pub(super) fn assert_sibling_file_not_dvd(dump: &Path, vts: &Path, mkv: &Path) {
    assert_eq!(dvd_disc_root(mkv), None);
    assert_eq!(resolve_open_media_path(mkv), mkv);
    assert!(!is_dvd_disc_path(mkv));
    assert!(!is_optical_disc_path(mkv));
    assert_eq!(dvd_disc_root(dump).as_deref(), Some(dump));
    assert_eq!(dvd_disc_root(vts).as_deref(), Some(dump));
    assert_eq!(
        dvd_disc_root(&vts.join("VTS_01_1.VOB")).as_deref(),
        Some(dump)
    );
}

/// Two title sets: VTS_01 (small first chapters) and VTS_02 (the large main feature).
pub(super) fn write_two_title_set(vts: &Path) {
    put_vob(&vts.join("VIDEO_TS.VOB"), 64);
    put_vob(&vts.join("VTS_01_0.VOB"), 128);
    put_vob(&vts.join("VTS_01_1.VOB"), 4096);
    put_vob(&vts.join("VTS_01_2.VOB"), 2048);
    put_vob(&vts.join("VTS_02_1.VOB"), 50_000);
    put_vob(&vts.join("VTS_02_2.VOB"), 50_000);
}

pub(super) fn assert_resolve_prefers_main_title(disc: &Path, vts: &Path) {
    assert_eq!(resolve_open_media_path(disc), vts.join("VTS_02_1.VOB"));
    assert_eq!(
        dvd_first_playable_vob(disc).as_deref(),
        Some(vts.join("VTS_02_1.VOB").as_path())
    );
    let p21 = vts.join("VTS_02_1.VOB");
    let title = crate::dvd_entity::vob_title_id(&p21);
    let title_vobs: Vec<_> = crate::dvd_entity::list_feature_vobs(&p21)
        .into_iter()
        .filter(|p| crate::dvd_entity::vob_title_id(p) == title)
        .collect();
    assert_eq!(title_vobs.len(), 2);
    assert_eq!(title_vobs[1], vts.join("VTS_02_2.VOB"));
}

pub(super) fn assert_explicit_chapter_wins(disc: &Path, vts: &Path) {
    let ch2 = vts.join("VTS_01_2.VOB");
    assert_eq!(resolve_open_media_path(disc), vts.join("VTS_02_1.VOB"));
    assert_eq!(resolve_open_media_path(&ch2), ch2);
}

pub(super) fn assert_broadcast_fps_table() {
    assert_eq!(dvd_vob_broadcast_fps(Some((768, 576))), Some(25.0));
    assert_eq!(dvd_vob_broadcast_fps(Some((720, 576))), Some(25.0));
    assert!((dvd_vob_broadcast_fps(Some((720, 480))).unwrap() - 30000.0 / 1001.0).abs() < 1e-6);
    assert!(dvd_vob_broadcast_fps(Some((1280, 720))).is_none());
}
