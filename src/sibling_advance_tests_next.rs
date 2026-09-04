#[test]
fn natural_episode_order() {
    let island = scratch_island("nat_ep", ScratchTmpOrder::First);
    let base = media_flat(&island);
    for name in ["ep2.mkv", "ep10.mkv", "ep1.mkv"] {
        seeded_video(&base, name);
    }
    let e1 = base.join("ep1.mkv");
    let e2 = base.join("ep2.mkv");
    let e10 = base.join("ep10.mkv");
    assert_same_path(&next_after_eof(&e1).unwrap(), &e2);
    assert_same_path(&next_after_eof(&e2).unwrap(), &e10);
    cleanup(&island);
}

#[test]
fn same_folder_next() {
    let island = scratch_island("sib1", ScratchTmpOrder::First);
    let base = media_flat(&island);
    let a = base.join("a.mp4");
    let b = base.join("b.mp4");
    fs::write(&a, b"x").unwrap();
    fs::write(&b, b"x").unwrap();
    assert_same_path(&next_after_eof(&a).unwrap(), &b);
    let _ = fs::remove_dir_all(&island);
}

#[test]
fn last_in_folder_goes_to_next_sibling_subdir() {
    let island = scratch_island("sib2", ScratchTmpOrder::First);
    let s1 = island.join("S1");
    let s2 = island.join("S2");
    ensure_dir(&s1);
    ensure_dir(&s2);
    let last = seeded_video(&s1, "e.mp4");
    seeded_video(&s2, "a.mp4");
    assert_same_path(&next_after_eof(&last).unwrap(), &s2.join("a.mp4"));
    cleanup(&island);
}

#[test]
fn last_in_last_sibling_stops() {
    let island = scratch_island("sib3", ScratchTmpOrder::Last);
    let base = &island;
    let s1 = base.join("S1");
    fs::create_dir_all(&s1).unwrap();
    let v1 = s1.join("e.mp4");
    fs::write(&v1, b"x").unwrap();
    assert!(next_after_eof(&v1).is_none());
    let _ = fs::remove_dir_all(island);
}

#[test]
fn skips_dir_without_videos() {
    let island = scratch_island("sib4", ScratchTmpOrder::First);
    let base = &island;
    for name in ["Show Season 1", "Show Season 2", "Show Season 3"] {
        fs::create_dir_all(base.join(name)).unwrap();
    }
    let va = base.join("Show Season 1").join("1.mp4");
    let vc = base.join("Show Season 3").join("1.mp4");
    fs::write(&va, b"x").unwrap();
    fs::write(&vc, b"x").unwrap();
    assert_same_path(&next_after_eof(&va).unwrap(), &vc);
    let _ = fs::remove_dir_all(island);
}

#[test]
fn vob_sibling_uses_shared_ext_list() {
    let island = scratch_island("sib_vob", ScratchTmpOrder::First);
    let base = media_flat(&island);
    let a = base.join("a.vob");
    let b = base.join("b.vob");
    fs::write(&a, b"x").unwrap();
    fs::write(&b, b"x").unwrap();
    assert_same_path(&next_after_eof(&a).unwrap(), &b);
    let _ = fs::remove_dir_all(&island);
}
