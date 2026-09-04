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

#[test]
fn resume_load_target_maps_global_to_chapter_vob() {
    let (base, vts) = pe_dvd_dir("thumb", &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
    let p1 = vts.join("VTS_02_1.VOB");
    let p2 = vts.join("VTS_02_2.VOB");
    let entity = PlaybackEntity::resolve(&p1);
    let (load, local) = entity
        .resume_load_target(&p1, 120.0, &global_dur_map(&entity, &p1, &p2))
        .expect("chapter target");
    assert!(crate::video_ext::paths_same_file(&load, &p2));
    assert!((local - 20.0).abs() < 1e-3);
    pe_remove(&base);
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
