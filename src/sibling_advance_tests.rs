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

#[test]
fn natural_episode_order() {
    let island = scratch_island("nat_ep", ScratchTmpOrder::First);
    let base = media_flat(&island);
    for name in ["ep2.mkv", "ep10.mkv", "ep1.mkv"] {
        fs::write(base.join(name), b"x").unwrap();
    }
    let e1 = base.join("ep1.mkv");
    let e2 = base.join("ep2.mkv");
    let e10 = base.join("ep10.mkv");
    let n1 = next_after_eof(&e1).unwrap();
    assert_same_path(&n1, &e2);
    let n2 = next_after_eof(&e2).unwrap();
    assert_same_path(&n2, &e10);
    let _ = fs::remove_dir_all(&island);
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
    let base = &island;
    let s1 = base.join("S1");
    let s2 = base.join("S2");
    fs::create_dir_all(&s1).unwrap();
    fs::create_dir_all(&s2).unwrap();
    let v1 = s1.join("e.mp4");
    let v2 = s2.join("a.mp4");
    fs::write(&v1, b"x").unwrap();
    fs::write(&v2, b"x").unwrap();
    let n = next_after_eof(&v1).unwrap();
    assert_same_path(&n, &v2);
    let _ = fs::remove_dir_all(island);
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
    let base = &island;
    let s1 = base.join("S1");
    let s2 = base.join("S2");
    fs::create_dir_all(&s1).unwrap();
    fs::create_dir_all(&s2).unwrap();
    let v1 = s1.join("a.mp4");
    let v2 = s2.join("z.mp4");
    fs::write(&v1, b"x").unwrap();
    fs::write(&v2, b"x").unwrap();
    assert_same_path(&prev_before_current(&v2).unwrap(), &v1);
    let _ = fs::remove_dir_all(island);
}

#[test]
fn skips_dir_without_videos() {
    let island = scratch_island("sib4", ScratchTmpOrder::First);
    let base = &island;
    for name in ["A", "B", "C"] {
        fs::create_dir_all(base.join(name)).unwrap();
    }
    let va = base.join("A").join("1.mp4");
    let vc = base.join("C").join("1.mp4");
    fs::write(&va, b"x").unwrap();
    fs::write(&vc, b"x").unwrap();
    assert_same_path(&next_after_eof(&va).unwrap(), &vc);
    let _ = fs::remove_dir_all(island);
}

#[test]
fn does_not_jump_to_parallel_folder_under_grandparent() {
    let island = scratch_island("para_show", ScratchTmpOrder::First);
    let show_a = island.join("ShowA");
    let show_b = island.join("ShowB");
    let s1a = show_a.join("S01");
    let s1b = show_b.join("S01");
    fs::create_dir_all(&s1a).unwrap();
    fs::create_dir_all(&s1b).unwrap();
    let va = s1a.join("only.mkv");
    let vb = s1b.join("other.mkv");
    fs::write(&va, b"x").unwrap();
    fs::write(&vb, b"x").unwrap();
    assert!(next_after_eof(&va).is_none());
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

fn write_min_dvd(disc: &Path, vob_name: &str) {
    let vts = disc.join("VIDEO_TS");
    fs::create_dir_all(&vts).unwrap();
    fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").unwrap();
    fs::write(vts.join(vob_name), b"v").unwrap();
}

#[test]
fn dvd_advances_to_sibling_disc_dir_not_next_vob() {
    let island = scratch_island("dvd_sib", ScratchTmpOrder::First);
    let d1 = island.join("DVD1");
    let d2 = island.join("DVD2");
    write_min_dvd(&d1, "VTS_02_1.VOB");
    write_min_dvd(&d1, "VTS_02_2.VOB");
    write_min_dvd(&d2, "VTS_02_1.VOB");
    let ch1 = d1.join("VIDEO_TS").join("VTS_02_1.VOB");
    let ch2 = d2.join("VIDEO_TS").join("VTS_02_1.VOB");
    assert_same_path(&next_after_eof(&ch1).unwrap(), &ch2);
    let mid = d1.join("VIDEO_TS").join("VTS_02_2.VOB");
    assert_same_path(&next_after_eof(&mid).unwrap(), &ch2);
    let p = prev_before_current(&ch2).unwrap();
    assert_same_path(&p, &ch1);
    let _ = fs::remove_dir_all(&island);
}
