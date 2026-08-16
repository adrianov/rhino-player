//! Non-recursive video listing and directory-open entry pick.

use lexical_sort::natural_lexical_cmp;
use std::fs;
use std::path::{Path, PathBuf};

use super::is_video_path;

/// Sorted video **files** directly under `dir` (no canonicalize — works on exFAT / network volumes).
pub(crate) fn list_videos_in_dir(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_video_path(p))
        .collect();
    v.sort_by(|a, b| {
        natural_lexical_cmp(
            a.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            b.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        )
    });
    Some(v)
}

pub(crate) fn dir_has_videos(dir: &Path) -> bool {
    dir.is_dir() && list_videos_in_dir(dir).is_some_and(|v| !v.is_empty())
}

/// Last video in natural order that still has a stored resume, else the first video.
pub(crate) fn folder_open_entry(dir: &Path) -> Option<PathBuf> {
    folder_open_pick(dir, |p| crate::db::resume_pos(p).is_some())
}

fn folder_open_pick(dir: &Path, has_position: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let videos = list_videos_in_dir(dir)?;
    videos
        .iter()
        .rev()
        .find(|p| has_position(p))
        .cloned()
        .or_else(|| videos.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!(
            "rhino-folder-scan-{label}-{}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_videos(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                fs::write(&p, b"x").unwrap();
                p
            })
            .collect()
    }

    #[test]
    fn natural_order_lists_episodes() {
        let dir = scratch("nat");
        write_videos(&dir, &["ep10.mkv", "ep2.mkv", "ep1.mkv"]);
        let listed = list_videos_in_dir(&dir).unwrap();
        let names: Vec<_> = listed
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .collect();
        assert_eq!(names, ["ep1.mkv", "ep2.mkv", "ep10.mkv"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_position_picks_first() {
        let dir = scratch("first");
        write_videos(&dir, &["ep2.mkv", "ep10.mkv", "ep1.mkv"]);
        let pick = folder_open_pick(&dir, |_| false).unwrap();
        assert_eq!(pick.file_name().and_then(|n| n.to_str()), Some("ep1.mkv"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_in_sort_with_position_wins() {
        let dir = scratch("resume");
        let files = write_videos(&dir, &["ep1.mkv", "ep2.mkv", "ep10.mkv"]);
        let mut have = HashSet::new();
        have.insert(files[0].clone());
        have.insert(files[1].clone());
        let pick = folder_open_pick(&dir, |p| have.contains(p)).unwrap();
        assert_eq!(pick.file_name().and_then(|n| n.to_str()), Some("ep2.mkv"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_dir_has_no_entry() {
        let dir = scratch("empty");
        assert!(folder_open_pick(&dir, |_| true).is_none());
        assert!(!dir_has_videos(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}
