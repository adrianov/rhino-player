//! Video filename extensions: Open dialog, sibling **Prev/Next**, and folder scanning share one list.

use std::path::Path;

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
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                let l = e.to_ascii_lowercase();
                SUFFIX.contains(&l.as_str())
            })
}
