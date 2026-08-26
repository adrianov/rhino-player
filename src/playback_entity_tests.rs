// Playback-entity behavior specs: path → entity resolution, shared DVD title keys,
// global resume mapping, and per-chapter stream identity. Fixtures build real temp DVD trees.

use super::*;
use std::fs;

/// Fresh DVD-folder fixture: `<base>/VIDEO_TS/VIDEO_TS.IFO` plus the named VOBs.
fn pe_dvd_dir(tag: &str, vobs: &[&str]) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("rhino-pe-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let vts = base.join("VIDEO_TS");
    fs::create_dir_all(&vts).expect("mkdir");
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
    for n in vobs {
        fs::write(vts.join(n), b"v").expect("write");
    }
    (base, vts)
}

fn pe_remove(base: &Path) {
    let _ = fs::remove_dir_all(base);
}

fn pe_key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

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

/// Card fixture: title set with a `VTS_02_0.IFO` index and two 1000/2000-byte VOBs.
fn card_fixture() -> (PathBuf, PathBuf, PathBuf) {
    let (base, vts) = pe_dvd_dir("card", &[]);
    fs::write(vts.join("VTS_02_0.IFO"), b"IFO").expect("ifo");
    for (n, size) in [("VTS_02_1.VOB", 1000), ("VTS_02_2.VOB", 2000)] {
        fs::write(vts.join(n), vec![0u8; size]).expect("write");
    }
    (base, vts.join("VTS_02_1.VOB"), vts.join("VTS_02_2.VOB"))
}

fn chapter_maps(p1: &Path, p2: &Path) -> (HashMap<String, f64>, HashMap<String, f64>) {
    let mut durs = HashMap::new();
    let mut tpos = HashMap::new();
    durs.insert(pe_key(p1), 100.0);
    durs.insert(pe_key(p2), 100.0);
    tpos.insert(pe_key(p2), 50.0);
    (durs, tpos)
}

#[test]
fn card_resume_uses_entity_not_chapter_local() {
    let (base, p1, p2) = card_fixture();
    let entity = db_path_for(&p1);
    let disc_key = std::fs::canonicalize(&base).unwrap_or(base.clone());
    assert!(crate::video_ext::paths_same_file(&entity, &disc_key));
    let (durs, tpos) = chapter_maps(&p1, &p2);
    let (resume, duration) = card_resume_duration(&p2, &durs, &tpos);
    assert!(duration > 100.0, "expected title duration, got {duration}");
    assert!(resume > 50.0 || resume == 0.0);
    pe_remove(&base);
}

/// Duration map seeding the entity row (150s) above chapter rows (100/50s).
fn global_dur_map(entity: &PlaybackEntity, p1: &Path, p2: &Path) -> HashMap<String, f64> {
    let mut durs = HashMap::new();
    durs.insert(pe_key(&entity.db_path()), 150.0);
    durs.insert(pe_key(p1), 100.0);
    durs.insert(pe_key(p2), 50.0);
    durs
}

#[test]
fn resume_load_target_maps_global_to_chapter_vob() {
    let (base, vts) = pe_dvd_dir("thumb", &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
    let p1 = vts.join("VTS_02_1.VOB");
    let p2 = vts.join("VTS_02_2.VOB");
    let entity = PlaybackEntity::resolve(&p1);
    let durs = global_dur_map(&entity, &p1, &p2);
    let (load, local) = entity
        .resume_load_target(&p1, 120.0, &durs)
        .expect("chapter target");
    assert!(crate::video_ext::paths_same_file(&load, &p2));
    assert!((local - 20.0).abs() < 1e-3);
    pe_remove(&base);
}

/// Stale-entity fixture: four `x`-filled VOBs sized `[100, 200, 300, 400]`; returns `(base, first vob)`.
fn sized_vobs_fixture(tag: &str) -> (PathBuf, PathBuf) {
    let (base, vts) = pe_dvd_dir(tag, &[]);
    for (i, n) in [100usize, 200, 300, 400].into_iter().enumerate() {
        fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; n]).expect("vob");
    }
    (base, vts.join("VTS_02_1.VOB"))
}

#[test]
fn card_resume_keeps_global_past_stale_entity_duration() {
    let (base, p1) = sized_vobs_fixture("stale");
    let entity = db_path_for(&p1);
    let ek = pe_key(&entity);
    let mut durs = HashMap::new();
    let mut tpos = HashMap::new();
    durs.insert(ek.clone(), 100.0);
    tpos.insert(ek, 130.0);
    durs.insert(pe_key(&p1), 100.0);
    let (resume, duration) = card_resume_duration(&entity, &durs, &tpos);
    assert!(resume > 100.0, "resume should stay global, got {resume}");
    assert!(
        duration >= resume,
        "duration {duration} should cover resume {resume}"
    );
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
