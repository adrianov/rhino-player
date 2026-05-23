use std::path::{Path, PathBuf};

use super::child_dirs_sorted;
use crate::video_ext;

pub(super) fn is_dvd_queue_path(path: &Path) -> bool {
    video_ext::is_dvd_vob_path(path) || video_ext::is_dvd_disc_path(path)
}

fn openable_dvd_chapter(disc: &Path) -> Option<PathBuf> {
    video_ext::dvd_first_playable_vob(disc).or_else(|| video_ext::dvd_main_chapter_vob(disc))
}

/// Next / previous sibling **disc directory** (folder containing `VIDEO_TS/`), not chapter `.vob`.
pub(super) fn dvd_disc_sibling(current: &Path, step: isize) -> Option<PathBuf> {
    let disc = video_ext::dvd_disc_root(current)?;
    let parent = disc.parent()?;
    let subs = child_dirs_sorted(parent);
    let idx = subs
        .iter()
        .position(|s| video_ext::paths_same_file(s, &disc))?;
    let candidates: Vec<&PathBuf> = match step.signum() {
        1 => subs.iter().skip(idx + 1).collect(),
        -1 => subs.iter().take(idx).rev().collect(),
        _ => return None,
    };
    for sdir in candidates {
        if !video_ext::is_dvd_disc_path(sdir) {
            continue;
        }
        if let Some(vob) = openable_dvd_chapter(sdir) {
            return Some(vob);
        }
    }
    None
}
