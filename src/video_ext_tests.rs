use super::dvd_pick::pick_main_dvd_vob;
use super::*;
use std::fs;
use std::path::Path;

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
fn pick_main_prefers_largest_title_by_bytes() {
    let base = std::env::temp_dir().join(format!("rhino-dvd-tie-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let vts = base.join("VIDEO_TS");
    fs::create_dir_all(&vts).expect("mkdir");
    fs::write(vts.join("VIDEO_TS.IFO"), b"IFO").expect("write");
    fs::write(vts.join("VTS_02_4.VOB"), vec![0u8; 1000]).expect("write");
    fs::write(vts.join("VTS_03_1.VOB"), vec![0u8; 500_000]).expect("write");
    assert_eq!(
        pick_main_dvd_vob(&vts).as_deref(),
        Some(vts.join("VTS_03_1.VOB").as_path())
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn fritt_dvd9_opens_main_vts01() {
    let disc = std::path::Path::new("/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9");
    let vts = disc.join("VIDEO_TS");
    if !vts.is_dir() {
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
    let p21 = vts.join("VTS_02_1.VOB");
    let title = crate::dvd_entity::vob_title_id(&p21);
    let title_vobs: Vec<_> = crate::dvd_entity::list_feature_vobs(&p21)
        .into_iter()
        .filter(|p| crate::dvd_entity::vob_title_id(p) == title)
        .collect();
    assert_eq!(title_vobs.len(), 2);
    assert_eq!(title_vobs[1], vts.join("VTS_02_2.VOB"));
    let ch2 = vts.join("VTS_01_2.VOB");
    assert_eq!(resolve_open_media_path(&disc), vts.join("VTS_02_1.VOB"));
    assert_eq!(resolve_open_media_path(&ch2), ch2);
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
