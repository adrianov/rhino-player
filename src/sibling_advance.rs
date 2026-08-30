//! Next local file after current ends at EOF. See `docs/features/07-sibling-folder-queue.md`.
//!
//! File and directory order uses the `lexical_sort` crate (`natural_lexical_cmp`): case-insensitive
//! Unicode folding to ASCII, plus **natural** digit runs (e.g. `ep2` before `ep10`). This is not
//! full [ICU] locale collation; for that see `icu_collator` (heavier).
//!
//! [ICU]: https://github.com/unicode-org/icu4x

use crate::video_ext;
use crate::video_ext::list_videos_in_dir;
use lexical_sort::{natural_lexical_cmp, PathSort};
use std::fs;
use std::path::{Path, PathBuf};

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
mod series {
    include!("sibling_advance_series.rs");
}
use dvd::{dvd_disc_sibling, is_dvd_queue_path};
use series::same_series_dirs;

/// Sibling entries after `idx` (step > 0) or before it, reversed (step < 0); [None] for step 0.
fn step_ordered(subs: &[PathBuf], idx: usize, step: isize) -> Option<Vec<&PathBuf>> {
    match step.signum() {
        1 => Some(subs.iter().skip(idx + 1).collect()),
        -1 => Some(subs.iter().take(idx).rev().collect()),
        _ => None,
    }
}

/// Sorted sibling dirs of `dir`'s parent, plus `dir`'s index among them.
fn dir_sibling_index(dir: &Path) -> Option<(Vec<PathBuf>, usize)> {
    let parent = dir.parent()?;
    let my = dir.file_name()?;
    let subs = child_dirs_sorted(parent);
    let idx = subs.iter().position(|s| s.file_name() == Some(my))?;
    Some((subs, idx))
}

/// First (forward) or last (backward) video in the adjacent sibling subfolder of `dir`,
/// under the same enclosing directory. Skips empty siblings and folders that belong to a
/// different series (season markers stripped; see `series::same_series_dirs`).
fn adjacent_sibling_dir_video(dir: &Path, step: isize) -> Option<PathBuf> {
    let (subs, idx) = dir_sibling_index(dir)?;
    step_ordered(&subs, idx, step)?
        .into_iter()
        .find_map(|sdir| same_series_pick(dir, sdir, step))
}

fn same_series_pick(from: &Path, sdir: &Path, step: isize) -> Option<PathBuf> {
    if !same_series_dirs(from, sdir) {
        return None;
    }
    sibling_video(sdir, step)
}

/// First video (forward step) or last video (backward step) in [sdir].
fn sibling_video(sdir: &Path, step: isize) -> Option<PathBuf> {
    if step > 0 {
        first_video_in_dir(sdir)
    } else {
        last_video_in_dir(sdir)
    }
}

/// Local file that follows `current` in the same **sorted** folder, then—if that folder is
/// exhausted—the first video in the next **same-series** sibling directory under the **same**
/// enclosing directory only (e.g. next season next to the current season). Unrelated show folders
/// beside it are skipped. There is **no** walk further up the tree. Used for EOF advance and the
/// **Next** control.
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
    adjacent_sibling_dir_video(dir, 1)
}

/// Symmetric to [next_after_eof]: the previous file in the same folder, or the **last** video in
/// the **previous** same-series sibling subfolder under the same enclosing directory only (no
/// extra walk-up; unrelated series folders are skipped).
pub(crate) fn prev_before_current(current: &Path) -> Option<PathBuf> {
    if is_dvd_queue_path(current) {
        return dvd_disc_sibling(current, -1);
    }
    if !current.is_file() {
        return None;
    }
    let current = current.to_path_buf();
    let dir = current.parent()?;
    let (videos, i) = index_in_dir_videos(&current, dir)?;
    if i > 0 {
        return Some(videos[i - 1].clone());
    }
    adjacent_sibling_dir_video(dir, -1)
}

/// Index of `current` in the sorted video list of `dir`, with that list.
fn index_in_dir_videos(current: &Path, dir: &Path) -> Option<(Vec<PathBuf>, usize)> {
    let videos = list_videos_in_dir(dir)?;
    let i = index_in_list(&videos, current)?;
    Some((videos, i))
}

#[cfg(test)]
mod tests {
    include!("sibling_advance_tests.rs");
    include!("sibling_advance_tests_dvd.rs");
}
