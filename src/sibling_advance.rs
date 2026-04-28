//! Next local file after current ends at EOF. See `docs/features/07-sibling-folder-queue.md`.
//!
//! File and directory order uses the `lexical_sort` crate (`natural_lexical_cmp`): case-insensitive
//! Unicode folding to ASCII, plus **natural** digit runs (e.g. `ep2` before `ep10`). This is not
//! full [ICU] locale collation; for that see `icu_collator` (heavier).
//!
//! [ICU]: https://github.com/unicode-org/icu4x

use crate::video_ext;
use lexical_sort::{natural_lexical_cmp, PathSort};
use std::fs;
use std::path::{Path, PathBuf};

/// Sorted, canonical, unique paths to video **files** directly under `dir`.
fn list_videos_in_dir(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| video_ext::is_video_path(p))
        .filter_map(|p| fs::canonicalize(&p).ok())
        .collect();
    v.path_sort_unstable(natural_lexical_cmp);
    Some(v)
}

fn index_in_list(list: &[PathBuf], current: &Path) -> Option<usize> {
    let c = fs::canonicalize(current).ok()?;
    list.iter().position(|p| p == &c)
}

/// Immediate subdirectories of `parent`, by natural+lexical name order.
fn child_dirs_sorted(parent: &Path) -> Vec<PathBuf> {
    let mut d: Vec<PathBuf> = match fs::read_dir(parent) {
        Ok(x) => x
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return Vec::new(),
    };
    d.path_sort_unstable(natural_lexical_cmp);
    d
}

/// First (sorted) video in `dir`, or [None] if none.
fn first_video_in_dir(dir: &Path) -> Option<PathBuf> {
    list_videos_in_dir(dir).and_then(|v| v.into_iter().next())
}

/// Last (sorted) video in `dir`, or [None] if none.
fn last_video_in_dir(dir: &Path) -> Option<PathBuf> {
    list_videos_in_dir(dir).and_then(|v| v.into_iter().last())
}

/// Local file that follows `current` in the same **sorted** folder, then the same sibling-folder
/// rules as on EOF. Used for both automatic advance at end and the **Next** control.
pub(crate) fn next_after_eof(current: &Path) -> Option<PathBuf> {
    let current = fs::canonicalize(current).ok()?;
    if !current.is_file() {
        return None;
    }
    let dir = current.parent()?;

    if let Some(videos) = list_videos_in_dir(dir) {
        if let Some(i) = index_in_list(&videos, &current) {
            if i + 1 < videos.len() {
                return Some(videos[i + 1].clone());
            }
        }
    }

    let mut folder = dir.to_path_buf();
    loop {
        let parent = folder.parent()?;
        let my = folder.file_name()?;
        let subs = child_dirs_sorted(parent);
        let idx = subs.iter().position(|s| s.file_name() == Some(my))?;
        for sdir in subs.iter().skip(idx + 1) {
            if let Some(f) = first_video_in_dir(sdir) {
                return Some(f);
            }
        }
        folder = parent.to_path_buf();
    }
}

/// Symmetric to [next_after_eof]: the previous file in the same folder, or the **last** video in the
/// **previous** sibling subfolder, walking up like forward navigation.
pub(crate) fn prev_before_current(current: &Path) -> Option<PathBuf> {
    let current = fs::canonicalize(current).ok()?;
    if !current.is_file() {
        return None;
    }
    let dir = current.parent()?;

    if let Some(videos) = list_videos_in_dir(dir) {
        if let Some(i) = index_in_list(&videos, &current) {
            if i > 0 {
                return Some(videos[i - 1].clone());
            }
        } else {
            return None;
        }
    } else {
        return None;
    }

    let mut folder = dir.to_path_buf();
    loop {
        let parent = folder.parent()?;
        let my = folder.file_name()?;
        let subs = child_dirs_sorted(parent);
        let idx = subs.iter().position(|s| s.file_name() == Some(my))?;
        for sdir in subs.iter().take(idx).rev() {
            if let Some(f) = last_video_in_dir(sdir) {
                return Some(f);
            }
        }
        folder = parent.to_path_buf();
    }
}

#[cfg(test)]
mod tests {
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

    /// Video files for “single folder” cases; island root has no loose videos, only this subdir.
    fn media_flat(island: &Path) -> PathBuf {
        let m = island.join("media");
        fs::create_dir_all(&m).unwrap();
        m
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
        assert_eq!(n1, fs::canonicalize(&e2).unwrap());
        let n2 = next_after_eof(&e2).unwrap();
        assert_eq!(n2, fs::canonicalize(&e10).unwrap());
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
        assert_eq!(fs::canonicalize(na).unwrap(), fs::canonicalize(&b).unwrap());
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
        assert_eq!(fs::canonicalize(n).unwrap(), fs::canonicalize(&v2).unwrap());
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
        let ca = fs::canonicalize(&a).unwrap();
        assert_eq!(prev_before_current(&b).unwrap(), ca);
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
        let p = prev_before_current(&v2).unwrap();
        assert_eq!(fs::canonicalize(p).unwrap(), fs::canonicalize(&v1).unwrap());
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
        let n = next_after_eof(&va).unwrap();
        assert_eq!(fs::canonicalize(n).unwrap(), fs::canonicalize(&vc).unwrap());
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
        let n = next_after_eof(&a).unwrap();
        assert_eq!(fs::canonicalize(n).unwrap(), fs::canonicalize(&b).unwrap());
        let _ = fs::remove_dir_all(&island);
    }
}
