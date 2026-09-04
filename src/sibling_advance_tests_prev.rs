#[test]
fn prev_same_folder() {
    let island = scratch_island("prev1", ScratchTmpOrder::First);
    let base = media_flat(&island);
    let a = base.join("a.mp4");
    let b = base.join("b.mp4");
    fs::write(&a, b"x").unwrap();
    fs::write(&b, b"x").unwrap();
    assert_same_path(&prev_before_current(&b).unwrap(), &a);
    assert!(prev_before_current(&a).is_none());
    let _ = fs::remove_dir_all(&island);
}

#[test]
fn prev_from_first_in_folder_to_previous_sibling_last() {
    let island = scratch_island("prev2", ScratchTmpOrder::First);
    let s1 = island.join("S1");
    let s2 = island.join("S2");
    ensure_dir(&s1);
    ensure_dir(&s2);
    seeded_video(&s1, "a.mp4");
    assert_same_path(
        &prev_before_current(&seeded_video(&s2, "z.mp4")).unwrap(),
        &s1.join("a.mp4"),
    );
    cleanup(&island);
}
