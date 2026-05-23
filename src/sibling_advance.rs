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

/// Sorted video **files** directly under `dir` (no canonicalize — works on exFAT / network volumes).
fn list_videos_in_dir(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| video_ext::is_video_path(p))
        .collect();
    v.sort_by(|a, b| {
        natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    Some(v)
}

fn index_in_list(list: &[PathBuf], current: &Path) -> Option<usize> {
    list.iter()
        .position(|p| video_ext::paths_same_file(p, current))
}

/// Immediate subdirectories of `parent`, by natural+lexical name order.
pub(super) fn child_dirs_sorted(parent: &Path) -> Vec<PathBuf> {
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

mod dvd {
    include!("sibling_advance_dvd.rs");
}
use dvd::{dvd_disc_sibling, is_dvd_queue_path};

/// Local file that follows `current` in the same **sorted** folder, then—if that folder is
/// exhausted—the first video in the next sibling directory under the **same** enclosing directory
/// only (e.g. next season next to the current season). There is **no** walk further up the tree, so
/// unrelated directories beside that grouping are never queued (e.g. another series under a shared
/// library folder). Used for EOF advance and the **Next** control.
pub(crate) fn next_after_eof(current: &Path) -> Option<PathBuf> {
    if is_dvd_queue_path(current) {
        return dvd_disc_sibling(current, 1);
    }
    if !current.is_file() {
        return None;
    }
    let current = current.to_path_buf();
    let dir = current.parent()?;

    if let Some(videos) = list_videos_in_dir(dir) {
        if let Some(i) = index_in_list(&videos, &current) {
            if i + 1 < videos.len() {
                return Some(videos[i + 1].clone());
            }
        }
    }

    let parent = dir.parent()?;
    let my = dir.file_name()?;
    let subs = child_dirs_sorted(parent);
    let idx = subs.iter().position(|s| s.file_name() == Some(my))?;
    for sdir in subs.iter().skip(idx + 1) {
        if let Some(f) = first_video_in_dir(sdir) {
            return Some(f);
        }
    }
    None
}

/// Symmetric to [next_after_eof]: the previous file in the same folder, or the **last** video in
/// the **previous** sibling subfolder under the same enclosing directory only (no extra walk-up).
pub(crate) fn prev_before_current(current: &Path) -> Option<PathBuf> {
    if is_dvd_queue_path(current) {
        return dvd_disc_sibling(current, -1);
    }
    if !current.is_file() {
        return None;
    }
    let current = current.to_path_buf();
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

    let parent = dir.parent()?;
    let my = dir.file_name()?;
    let subs = child_dirs_sorted(parent);
    let idx = subs.iter().position(|s| s.file_name() == Some(my))?;
    for sdir in subs.iter().take(idx).rev() {
        if let Some(f) = last_video_in_dir(sdir) {
            return Some(f);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    include!("sibling_advance_tests.rs");
}
