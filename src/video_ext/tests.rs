use super::dvd_pick::pick_main_dvd_vob;
use super::*;
use std::fs;
use std::path::Path;

mod dvd_fixtures;

use dvd_fixtures::{
    assert_broadcast_fps_table, assert_explicit_chapter_wins, assert_resolve_prefers_main_title,
    assert_sibling_file_not_dvd, make_video_ts, put_bytes, put_vob, scratch, write_two_title_set,
};

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
    let base = scratch("dvd");
    let disc = base.join("DVD1");
    let vts = make_video_ts(&disc.join("VIDEO_TS"));
    assert_eq!(dvd_disc_root(&disc).as_deref(), Some(disc.as_path()));
    assert_eq!(dvd_disc_root(&vts).as_deref(), Some(disc.as_path()));
    let mixed = base.join("Mgnoveniy");
    let vts2 = make_video_ts(&mixed.join("Video_ts"));
    assert_eq!(dvd_disc_root(&mixed).as_deref(), Some(mixed.as_path()));
    assert_eq!(dvd_disc_root(&vts2).as_deref(), Some(mixed.as_path()));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn pick_main_prefers_largest_title_by_bytes() {
    let base = scratch("dvd-tie");
    let vts = make_video_ts(&base.join("VIDEO_TS"));
    put_vob(&vts.join("VTS_02_4.VOB"), 1000);
    put_vob(&vts.join("VTS_03_1.VOB"), 500_000);
    assert_eq!(
        pick_main_dvd_vob(&vts).as_deref(),
        Some(vts.join("VTS_03_1.VOB").as_path())
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn fritt_dvd9_opens_main_vts01() {
    let disc = std::path::Path::new("/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9");
    if !disc.join("VIDEO_TS").is_dir() {
        return;
    }
    let main = dvd_main_chapter_vob(disc).expect("main");
    assert_eq!(
        main.file_name().and_then(|n| n.to_str()),
        Some("VTS_01_1.VOB"),
        "full-size first chapter holds splash, got {}",
        main.display()
    );
}

#[test]
fn ordinary_file_beside_video_ts_is_not_dvd() {
    let base = scratch("dvd-sib");
    let dump = base.join("Torrents");
    let vts = make_video_ts(&dump.join("VIDEO_TS"));
    put_vob(&vts.join("VTS_01_1.VOB"), 4096);
    let mkv = dump.join("clip.mkv");
    put_bytes(&mkv, b"x");
    assert_sibling_file_not_dvd(&dump, &vts, &mkv);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn dvd_resolve_opens_main_title_first_chapter() {
    let base = scratch("dvd-vob");
    let disc = base.join("DVD1");
    let vts = make_video_ts(&disc.join("VIDEO_TS"));
    write_two_title_set(&vts);
    assert_resolve_prefers_main_title(&disc, &vts);
    assert_explicit_chapter_wins(&disc, &vts);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paths_same_file_ignores_video_ts_casing_without_canonicalize() {
    let a = Path::new("/Volumes/Disc/DVD/Video_ts/VTS_02_1.VOB");
    let b = Path::new("/Volumes/Disc/DVD/VIDEO_TS/VTS_02_1.VOB");
    assert!(paths_same_file(a, b));
    let c = Path::new("/Volumes/Disc/DVD/VIDEO_TS/VTS_02_2.VOB");
    assert!(!paths_same_file(a, c));
}

#[test]
fn dvd_vob_path_and_broadcast_fps() {
    let base = scratch("dvd-fps");
    let vts = base.join("Video_ts");
    fs::create_dir_all(&vts).expect("mkdir");
    let ch = vts.join("VTS_02_1.VOB");
    put_bytes(&ch, b"x");
    assert!(is_dvd_vob_path(&ch));
    assert!(!is_dvd_vob_path(&base.join("clip.mkv")));
    assert_broadcast_fps_table();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn ordinary_folder_opens_first_video_without_resume() {
    let base = scratch("folder-open");
    fs::create_dir_all(&base).expect("mkdir");
    put_bytes(&base.join("ep2.mkv"), b"x");
    put_bytes(&base.join("ep10.mkv"), b"x");
    put_bytes(&base.join("ep1.mkv"), b"x");
    assert!(is_openable_media_path(&base));
    assert_eq!(
        resolve_open_media_path(&base)
            .file_name()
            .and_then(|n| n.to_str()),
        Some("ep1.mkv")
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn dctmp_in_progress_download_is_openable() {
    let base = std::env::temp_dir().join(format!("rhino-dctmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("mkdir");
    let stub = base.join("clip.mkv.dctmp");
    fs::write(&stub, b"x").expect("write");
    assert!(is_video_path(&stub));
    assert!(is_openable_media_path(&stub));
    assert!(SUFFIX.contains(&"dctmp"));
    let _ = fs::remove_dir_all(&base);
}
