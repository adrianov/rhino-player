//! Strip incomplete-download wrappers from a basename before humanizing.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// True for Direct Connect in-progress paths (`*.dctmp`, any case).
pub(crate) fn is_incomplete_download_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("dctmp"))
}

/// `name.mkv.<id>.dctmp` → `name.mkv` (normal extension strip runs next).
pub(super) fn peel_download_temp(name: &str) -> String {
    let mut s = name.to_string();
    if !strip_ci_suffix(&mut s, ".dctmp") {
        return s;
    }
    // Direct Connect appends a long base32 id (Tiger Tree Hash, often 39 chars).
    static ID_TAIL: OnceLock<Regex> = OnceLock::new();
    let re = ID_TAIL.get_or_init(|| Regex::new(r"(?i)\.[a-z2-7]{16,}$").expect("id_tail"));
    if let Some(m) = re.find(&s) {
        s.truncate(m.start());
    }
    s
}

/// Same-folder finished file for an incomplete Direct Connect path, when it exists on disk.
pub(crate) fn finished_download_path(incomplete: &Path) -> Option<PathBuf> {
    let name = incomplete.file_name()?.to_str()?;
    let finished_name = peel_download_temp(name);
    if finished_name == name {
        return None;
    }
    let finished = incomplete.parent()?.join(finished_name);
    finished.is_file().then_some(finished)
}

fn strip_ci_suffix(s: &mut String, suffix: &str) -> bool {
    let sl = suffix.len();
    if s.len() >= sl && s[s.len() - sl..].eq_ignore_ascii_case(suffix) {
        s.truncate(s.len() - sl);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn detects_dctmp_suffix() {
        assert!(is_incomplete_download_path(Path::new(
            "/dl/clip.mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp"
        )));
        assert!(is_incomplete_download_path(Path::new("clip.DCTMP")));
        assert!(!is_incomplete_download_path(Path::new("clip.mkv")));
    }

    #[test]
    fn strips_id_and_dctmp() {
        assert_eq!(
            peel_download_temp(
                "Связь (Coherence, 2013, 1080p).mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp"
            ),
            "Связь (Coherence, 2013, 1080p).mkv"
        );
    }

    #[test]
    fn strips_plain_dctmp() {
        assert_eq!(peel_download_temp("clip.mkv.dctmp"), "clip.mkv");
    }

    #[test]
    fn leaves_finished_names() {
        assert_eq!(peel_download_temp("Movie.Name.2020.mkv"), "Movie.Name.2020.mkv");
    }

    #[test]
    fn finished_path_when_sibling_exists() {
        let base = std::env::temp_dir().join(format!(
            "rhino-dctmp-done-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        let finished = base.join("clip.mkv");
        let incomplete =
            base.join("clip.mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp");
        fs::File::create(&finished).unwrap().write_all(b"x").unwrap();
        assert_eq!(
            finished_download_path(&incomplete).as_deref(),
            Some(finished.as_path())
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn finished_path_none_without_sibling_or_suffix() {
        let base = std::env::temp_dir().join(format!(
            "rhino-dctmp-miss-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        let incomplete =
            base.join("clip.mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp");
        assert!(finished_download_path(&incomplete).is_none());
        assert!(finished_download_path(&base.join("clip.mkv")).is_none());
        let _ = fs::remove_dir_all(&base);
    }
}
