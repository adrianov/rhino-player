//! Continue-card resolution class (`1080p`, `720p`, `2160p`, …).

use std::path::Path;

/// Progressive-scan label for a continue card: stored decode size, else a path-segment tag.
pub(super) fn quality_tag_for(path: &Path) -> Option<String> {
    if let Some((w, h)) = crate::db::media_decode_size(path) {
        return Some(quality_from_dims(w, h));
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .and_then(|n| quality_from_name(&n))
}

/// Map coded width/height to a common `Np` class (cinemascope 1920×800 stays 1080p).
fn quality_from_dims(w: i32, h: i32) -> String {
    format!("{}p", standard_p(w, h))
}

/// `(min_long, min_short, p)` — first match wins. 2560×1080 stays 1080p (short misses 1440).
const P_STEPS: &[(i32, i32, i32)] = &[
    (3800, 1600, 2160),
    (2500, 1300, 1440),
    (1900, 700, 1080),
    (1200, 500, 720),
    (0, 576, 576),
    (0, 480, 480),
];

fn standard_p(w: i32, h: i32) -> i32 {
    let long = w.max(h);
    let short = w.min(h);
    P_STEPS
        .iter()
        .find(|&&(ml, ms, _)| long >= ml && short >= ms)
        .map(|&(_, _, p)| p)
        .unwrap_or_else(|| short.max(1))
}

/// Release-style tokens in the basename (`WEB-DL1080p`, `4K`, `UHD`).
fn quality_from_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    for (needle, tag) in NAME_TAGS {
        if tag_in(&lower, needle) {
            return Some((*tag).to_string());
        }
    }
    None
}

const NAME_TAGS: &[(&str, &str)] = &[
    ("4320p", "4320p"),
    ("2160p", "2160p"),
    ("1440p", "1440p"),
    ("1080p", "1080p"),
    ("720p", "720p"),
    ("576p", "576p"),
    ("480p", "480p"),
    ("360p", "360p"),
    ("8k", "4320p"),
    ("4k", "2160p"),
    ("uhd", "2160p"),
];

fn tag_in(hay: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(i) = hay[start..].find(needle) {
        let at = start + i;
        let end = at + needle.len();
        let after_ok = end >= hay.len() || !hay.as_bytes()[end].is_ascii_alphanumeric();
        if after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{quality_from_dims, quality_from_name, quality_tag_for};
    use std::path::Path;

    #[test]
    fn dims_map_to_common_classes() {
        assert_eq!(quality_from_dims(1920, 1080), "1080p");
        assert_eq!(quality_from_dims(1920, 804), "1080p");
        assert_eq!(quality_from_dims(1280, 720), "720p");
        assert_eq!(quality_from_dims(3840, 2160), "2160p");
        assert_eq!(quality_from_dims(2560, 1440), "1440p");
        assert_eq!(quality_from_dims(2560, 1080), "1080p");
        assert_eq!(quality_from_dims(720, 480), "480p");
        assert_eq!(quality_from_dims(720, 576), "576p");
        assert_eq!(quality_from_dims(1080, 1920), "1080p");
    }

    #[test]
    fn name_picks_release_tokens() {
        assert_eq!(
            quality_from_name("Show.S01E01.1080p.WEB-DL.mkv").as_deref(),
            Some("1080p")
        );
        assert_eq!(
            quality_from_name("Movie.HDTV1080p.mkv").as_deref(),
            Some("1080p")
        );
        assert_eq!(quality_from_name("Film.4K.Remux.mkv").as_deref(), Some("2160p"));
        assert_eq!(quality_from_name("Clip.720p.mp4").as_deref(), Some("720p"));
        assert!(quality_from_name("plain-clip.mkv").is_none());
        assert!(quality_from_name("4kids-movie.mkv").is_none());
    }

    #[test]
    fn path_falls_back_to_basename_when_store_empty() {
        let p = Path::new("/tmp/never-stored-Show.S02E03.720p.mkv");
        assert_eq!(quality_tag_for(p).as_deref(), Some("720p"));
    }
}
