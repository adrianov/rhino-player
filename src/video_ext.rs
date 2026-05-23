//! Video filename extensions: Open dialog, sibling **Prev/Next**, and folder scanning share one list.
//! Blu-ray / AVCHD **BDMV** layout detection lives here too ([bluray_disc_root], [is_bluray_disc_path]).

use std::path::{Path, PathBuf};

/// Lowercase extensions (no leading dot) for “is this a video file?” in a directory.
/// Kept in sync with the **Open Video** file filter; extend here only.
/// **`ts`**: MPEG transport stream; pair with `video/mp2t` in `data/applications/*.desktop` for “Open with”.
pub const SUFFIX: &[&str] = &[
    "3g2", "3gp", "asf", "avi", "divx", "dvr-ms", "f4v", "flv", "h264", "h265", "hevc", "m2ts",
    "m4v", "mkv", "mov", "mpeg", "mpg", "mp4", "mts", "mxf", "nsv", "ogv", "rmp4", "ts", "vob",
    "webm", "wmv", "xvid", "y4m", "yuv",
];

/// `true` for a regular file whose extension is in [SUFFIX] (case-insensitive).
pub fn is_video_path(p: &Path) -> bool {
    p.is_file()
        && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            let l = e.to_ascii_lowercase();
            SUFFIX.contains(&l.as_str())
        })
}

const MOVIE_OBJECT_NAMES: &[&str] = &["MovieObject.bdmv", "MOVIEOBJ.BDM"];

fn movie_object_in(dir: &Path) -> bool {
    MOVIE_OBJECT_NAMES
        .iter()
        .any(|name| dir.join(name).is_file())
}

/// Disc root for a Blu-ray / AVCHD **BDMV** tree (parent of `BDMV/` when applicable).
pub fn bluray_disc_root(path: &Path) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if path.is_file() {
        path.parent().map(|p| vec![p.to_path_buf()])?
    } else {
        let mut v = vec![path.to_path_buf()];
        let bdmv = path.join("BDMV");
        if bdmv.is_dir() {
            v.push(bdmv);
        }
        v
    };
    for root in candidates {
        if !movie_object_in(&root) {
            continue;
        }
        let disc = if root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("BDMV"))
        {
            root.parent()?.to_path_buf()
        } else {
            root
        };
        return Some(disc);
    }
    None
}

/// `true` when `path` is a disc root, `BDMV/` package dir, or `MovieObject.bdmv`.
pub fn is_bluray_disc_path(path: &Path) -> bool {
    bluray_disc_root(path).is_some()
}

/// Local path acceptable for **Open**, CLI boot, and external `open` handlers.
pub fn is_openable_media_path(path: &Path) -> bool {
    is_video_path(path) || is_bluray_disc_path(path)
}

/// Normalize picker / argv paths before `loadfile` (Blu-ray trees → disc root).
pub fn resolve_open_media_path(path: &Path) -> PathBuf {
    bluray_disc_root(path).unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn bluray_root_from_disc_and_bdmv_package() {
        let base = std::env::temp_dir().join(format!("rhino-bluray-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let disc = base.join("Disc");
        let bdmv = disc.join("BDMV");
        fs::create_dir_all(&bdmv).expect("mkdir");
        fs::write(bdmv.join("MovieObject.bdmv"), b"MOBJ0200").expect("write");
        assert_eq!(bluray_disc_root(&disc).as_deref(), Some(disc.as_path()));
        assert_eq!(bluray_disc_root(&bdmv).as_deref(), Some(disc.as_path()));
        assert_eq!(
            bluray_disc_root(&bdmv.join("MovieObject.bdmv")).as_deref(),
            Some(disc.as_path())
        );
        let _ = fs::remove_dir_all(&base);
    }
}
