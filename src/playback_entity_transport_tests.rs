use std::fs;

use super::PlaybackEntity;

/// Fresh `(dvd base, mkv base)` temp dirs, both cleared first.
fn tbar_dirs() -> (std::path::PathBuf, std::path::PathBuf) {
    let base = std::env::temp_dir().join(format!("rhino-pe-tbar-{}", std::process::id()));
    let mkv_base = std::env::temp_dir().join(format!("rhino-pe-tbar-mkv-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&mkv_base);
    fs::create_dir_all(&base).expect("mkdir");
    fs::create_dir_all(&mkv_base).expect("mkdir mkv");
    (base, mkv_base)
}

/// Writes `VIDEO_TS/VIDEO_TS.IFO` plus two chapter VOBs; returns the first VOB.
fn tbar_dvd_tree(base: &std::path::Path) -> std::path::PathBuf {
    let vts = base.join("VIDEO_TS");
    fs::create_dir_all(&vts).expect("mkdir vts");
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
    for n in ["VTS_02_1.VOB", "VTS_02_2.VOB"] {
        fs::write(vts.join(n), b"v").expect("write");
    }
    vts.join("VTS_02_1.VOB")
}

fn tbar_mkv(mkv_base: &std::path::Path) -> std::path::PathBuf {
    let mkv = mkv_base.join("clip.mkv");
    fs::write(&mkv, b"x").expect("mkv");
    mkv
}

/// Bar over the two chapter VOBs (100 s + 200 s).
fn tbar_bar(p1: &std::path::Path) -> crate::dvd_vob_timeline::DvdBarState {
    let mut map = std::collections::HashMap::new();
    map.insert(p1.to_string_lossy().into_owned(), 100.0);
    map.insert(
        p1.with_file_name("VTS_02_2.VOB")
            .to_string_lossy()
            .into_owned(),
        200.0,
    );
    crate::dvd_vob_timeline::DvdBarState::build_with_map(p1, 100.0, &map).expect("bar")
}

fn assert_single_file_ignores_bar(
    file_ent: &PlaybackEntity,
    mkv: &std::path::Path,
    bar: &crate::dvd_vob_timeline::DvdBarState,
) {
    assert!(!file_ent.uses_dvd_bar_cache());
    assert_eq!(
        file_ent.transport_bar(mkv, 12.0, 3600.0, Some(bar), None),
        (3600.0, 12.0)
    );
}

fn assert_long_file_uncapped(file_ent: &PlaybackEntity, mkv: &std::path::Path) {
    // DVD per-`.vob` cap must not zero long single-file durations (e.g. 4+ h MKV).
    let long_sec = crate::dvd_vob_timeline::MAX_VOB_DUR_SEC + 743.0;
    assert_eq!(
        file_ent.transport_bar(mkv, 100.0, long_sec, None, None),
        (long_sec, 100.0)
    );
}

#[test]
fn transport_bar_ignores_dvd_bar_for_single_file() {
    let (base, mkv_base) = tbar_dirs();
    let p1 = tbar_dvd_tree(&base);
    let mkv = tbar_mkv(&mkv_base);
    let bar = tbar_bar(&p1);
    let file_ent = PlaybackEntity::resolve(&mkv);
    assert_single_file_ignores_bar(&file_ent, &mkv, &bar);
    assert_long_file_uncapped(&file_ent, &mkv);
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
