#[test]
fn standalone_is_single_file_entity() {
    let base = std::env::temp_dir().join(format!("rhino-pe-file-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("mkdir");
    let f = base.join("clip.mkv");
    fs::write(&f, b"x").expect("write");
    let ent = PlaybackEntity::resolve(&f);
    assert!(!ent.has_unified_timeline());
    assert_eq!(ent.db_path(), fs::canonicalize(&f).unwrap());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn dvd_disc_folder_maps_to_title_entity() {
    let (base, vts) = pe_dvd_dir("disc", &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
    let from_disc = PlaybackEntity::resolve(&base);
    assert!(from_disc.has_unified_timeline());
    let disc_key = std::fs::canonicalize(&base).unwrap_or_else(|_| base.clone());
    assert_eq!(from_disc.db_path(), disc_key);
    assert_eq!(
        PlaybackEntity::resolve(&vts.join("VTS_02_1.VOB")).db_path(),
        from_disc.db_path()
    );
    pe_remove(&base);
}

#[test]
fn dvd_chapters_share_entity_key() {
    let (base, vts) = pe_dvd_dir("dvd", &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
    let e1 = PlaybackEntity::resolve(&vts.join("VTS_02_1.VOB"));
    let e2 = PlaybackEntity::resolve(&vts.join("VTS_02_2.VOB"));
    assert!(e1.has_unified_timeline());
    assert_eq!(e1.db_path(), e2.db_path());
    pe_remove(&base);
}

#[test]
fn title_set_streams_match_on_every_chapter() {
    let vob = Path::new(
        "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD2/Video_ts/VTS_02_1.VOB",
    );
    if !vob.is_file() {
        return;
    }
    let p2 = vob.with_file_name("VTS_02_2.VOB");
    if !p2.is_file() {
        return;
    }
    let e1 = PlaybackEntity::resolve(vob);
    let e2 = PlaybackEntity::resolve(&p2);
    let s1 = e1.title_set_streams(vob);
    let s2 = e2.title_set_streams(&p2);
    assert!(s1.is_some());
    assert_eq!(s1, s2);
}
