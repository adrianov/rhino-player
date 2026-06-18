use std::fs;

use super::PlaybackEntity;

#[test]
fn transport_bar_ignores_dvd_bar_for_single_file() {
    let base = std::env::temp_dir().join(format!("rhino-pe-tbar-{}", std::process::id()));
    let mkv_base = std::env::temp_dir().join(format!("rhino-pe-tbar-mkv-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&mkv_base);
    fs::create_dir_all(&base).expect("mkdir");
    fs::create_dir_all(&mkv_base).expect("mkdir mkv");
    let vts = base.join("VIDEO_TS");
    fs::create_dir_all(&vts).expect("mkdir vts");
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
    for n in ["VTS_02_1.VOB", "VTS_02_2.VOB"] {
        fs::write(vts.join(n), b"v").expect("write");
    }
    let p1 = vts.join("VTS_02_1.VOB");
    let mkv = mkv_base.join("clip.mkv");
    fs::write(&mkv, b"x").expect("mkv");
    let mut map = std::collections::HashMap::new();
    map.insert(p1.to_string_lossy().into_owned(), 100.0);
    map.insert(
        vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
        200.0,
    );
    let bar = crate::dvd_vob_timeline::DvdBarState::build_with_map(&p1, 100.0, &map).expect("bar");
    let file_ent = PlaybackEntity::resolve(&mkv);
    assert!(!file_ent.uses_dvd_bar_cache());
    assert_eq!(
        file_ent.transport_bar(&mkv, 12.0, 3600.0, Some(&bar), None),
        (3600.0, 12.0)
    );
    // DVD per-`.vob` cap must not zero long single-file durations (e.g. 4+ h MKV).
    let long_sec = crate::dvd_vob_timeline::MAX_VOB_DUR_SEC + 743.0;
    assert_eq!(
        file_ent.transport_bar(&mkv, 100.0, long_sec, None, None),
        (long_sec, 100.0)
    );
    let dvd_ent = PlaybackEntity::resolve(&p1);
    assert!(dvd_ent.uses_dvd_bar_cache());
    assert_eq!(dvd_ent.transport_duration_from_bar(&p1, &bar), Some(300.0));
    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&mkv_base);
}

#[test]
fn unified_timeline_chapter_requires_title_entity() {
    let base = std::env::temp_dir().join(format!("rhino-pe-utc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("mkdir");
    let mkv = base.join("clip.mkv");
    fs::write(&mkv, b"x").expect("mkv");
    assert!(!PlaybackEntity::resolve(&mkv).uses_dvd_bar_cache());
    let _ = fs::remove_dir_all(&base);
}
