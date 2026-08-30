use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Real `temp_dir` layout for tests. [ScratchTmpOrder] avoids picking up unrelated videos when
/// `prev_before_current` / `next_after_eof` walk up to `/tmp`: **First** = no lexically earlier
/// peers scanned; **Last** = no later peers scanned.
#[derive(Clone, Copy)]
enum ScratchTmpOrder {
    First,
    Last,
}

fn scratch_island(label: &str, order: ScratchTmpOrder) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let prefix = match order {
        ScratchTmpOrder::First => "!rhino_sib",
        ScratchTmpOrder::Last => "zzz_rhino_sib",
    };
    let p = std::env::temp_dir().join(format!(
        "{}_{}_{}_{:?}_{}",
        prefix,
        label,
        std::process::id(),
        std::thread::current().id(),
        nanos
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn media_flat(island: &Path) -> PathBuf {
    let m = island.join("media");
    fs::create_dir_all(&m).unwrap();
    m
}

fn assert_same_path(got: &Path, want: &Path) {
    assert!(
        video_ext::paths_same_file(got, want),
        "got {} want {}",
        got.display(),
        want.display()
    );
}

/// Creates [path] as a directory.
fn ensure_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

/// Creates parent dir if needed, writes an empty video placeholder, returns the file path.
fn seeded_video(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    touch_file(&p);
    p
}

fn touch_file(path: &Path) {
    fs::write(path, b"x").unwrap();
}

/// Removes a scratch island, ignoring absence.
fn cleanup(island: &Path) {
    let _ = fs::remove_dir_all(island);
}

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
    let na = next_after_eof(&a).unwrap();
    assert_same_path(&na, &b);
    let _ = fs::remove_dir_all(&island);
}

#[test]
fn last_in_folder_goes_to_next_sibling_subdir() {
    let island = scratch_island("sib2", ScratchTmpOrder::First);
    let s1 = island.join("S1");
    let s2 = island.join("S2");
    ensure_dir(&s1);
    ensure_dir(&s2);
    let v1 = seeded_video(&s1, "e.mp4");
    let v2 = seeded_video(&s2, "a.mp4");
    assert_same_path(&next_after_eof(&v1).unwrap(), &v2);
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
    let v1 = seeded_video(&s1, "a.mp4");
    let v2 = seeded_video(&s2, "z.mp4");
    assert_same_path(&prev_before_current(&v2).unwrap(), &v1);
    cleanup(&island);
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
    let v1 = seeded_video(&s1, "e.mkv");
    let v2 = seeded_video(&s2, "a.mkv");
    assert_same_path(&next_after_eof(&v1).unwrap(), &v2);
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
