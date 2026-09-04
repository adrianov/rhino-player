#[test]
fn does_not_advance_to_different_series_sibling() {
    let island = scratch_island("series_cross", ScratchTmpOrder::First);
    let hod = island.join("House of the Dragon Season 2");
    let legion = island.join("Legion Season 1");
    ensure_dir(&hod);
    ensure_dir(&legion);
    let last = seeded_video(&hod, "e10.mkv");
    seeded_video(&legion, "e01.mkv");
    assert!(next_after_eof(&last).is_none());
    cleanup(&island);
}

#[test]
fn advances_to_next_named_season_of_same_series() {
    let island = scratch_island("series_next", ScratchTmpOrder::First);
    let s1 = island.join("House of the Dragon Season 1");
    let s2 = island.join("House of the Dragon Season 2");
    ensure_dir(&s1);
    ensure_dir(&s2);
    seeded_video(&s2, "a.mkv");
    assert_same_path(
        &next_after_eof(&seeded_video(&s1, "e.mkv")).unwrap(),
        &s2.join("a.mkv"),
    );
    cleanup(&island);
}

#[test]
fn advances_across_bare_sxx_season_folders() {
    let island = scratch_island("bare_sxx", ScratchTmpOrder::First);
    let show = island.join("Some Show");
    let s1 = show.join("S01");
    let s2 = show.join("S02");
    ensure_dir(&s1);
    ensure_dir(&s2);
    let v1 = seeded_video(&s1, "e.mkv");
    let v2 = seeded_video(&s2, "a.mkv");
    assert_same_path(&next_after_eof(&v1).unwrap(), &v2);
    assert_same_path(&prev_before_current(&v2).unwrap(), &v1);
    cleanup(&island);
}

#[test]
fn does_not_jump_to_parallel_folder_under_grandparent() {
    let island = scratch_island("para_show", ScratchTmpOrder::First);
    let show_a = island.join("ShowA");
    let show_b = island.join("ShowB");
    let s1a = show_a.join("S01");
    let s1b = show_b.join("S01");
    ensure_dir(&s1a);
    ensure_dir(&s1b);
    let va = seeded_video(&s1a, "only.mkv");
    seeded_video(&s1b, "other.mkv");
    assert!(next_after_eof(&va).is_none());
    cleanup(&island);
}
