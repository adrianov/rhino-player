//! Classify local open failures and produce short user-facing copy for the continue-grid notice.

use std::io::Read;
use std::path::Path;

/// Sample window for hollow / preallocated torrent stubs (all-zero prefix).
const HOLLOW_PREFIX_BYTES: usize = 64 * 1024;

/// User-visible lines shown in the open-failure notice toast.
pub mod msg {
    /// Zero-byte or all-zero preallocated download with no container bytes.
    pub const EMPTY_OR_INCOMPLETE: &str = "Nothing to play — this file looks empty or incomplete.";
    /// Demuxer rejected the file (corrupt, truncated, wrong type, etc.).
    pub const UNREADABLE_MEDIA: &str =
        "Can't play this file — Rhino couldn't read any media from it.";
    /// Path missing after resolve.
    pub const MISSING: &str = "Can't open this file — it may have been moved or deleted.";
    /// Generic fallback when only a low-level error string is available.
    pub const GENERIC: &str = "Couldn't open this file.";
}

/// Fast preflight before `loadfile`. Returns a notice string when opening should not proceed.
pub fn preflight_user_message(path: &Path) -> Option<&'static str> {
    if crate::video_ext::is_optical_disc_path(path) {
        return None;
    }
    if !path.exists() {
        return Some(msg::MISSING);
    }
    if path.is_dir() {
        return Some(msg::UNREADABLE_MEDIA);
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    if meta.len() == 0 {
        return Some(msg::EMPTY_OR_INCOMPLETE);
    }
    if looks_hollow_file(path, meta.len()) {
        return Some(msg::EMPTY_OR_INCOMPLETE);
    }
    None
}

/// Message after mpv `EndFile` with error reason (async demux failure).
pub fn message_for_demux_error(path: Option<&Path>) -> &'static str {
    if let Some(p) = path {
        if let Some(m) = preflight_user_message(p) {
            return m;
        }
    }
    msg::UNREADABLE_MEDIA
}

/// True when this notice means the path should leave the continue list.
pub fn should_drop_from_continue(msg: &str) -> bool {
    matches!(
        msg,
        msg::EMPTY_OR_INCOMPLETE | msg::UNREADABLE_MEDIA | msg::MISSING
    )
}

/// Map a `try_load` / `loadfile` error string to notice copy.
pub fn message_for_load_err(err: &str, path: &Path) -> String {
    if let Some(m) = preflight_user_message(path) {
        return m.to_string();
    }
    let e = err.trim();
    if e.is_empty() {
        return msg::GENERIC.to_string();
    }
    // Keep concise shell messages; scrub noisy Debug dumps from libmpv.
    if e.starts_with('[') || e.contains("Error(") || e.contains("MPV_") {
        return msg::UNREADABLE_MEDIA.to_string();
    }
    if e.len() > 120 {
        return msg::GENERIC.to_string();
    }
    e.to_string()
}

/// True when the file is a zero-filled stub (typical incomplete torrent preallocation).
fn looks_hollow_file(path: &Path, len: u64) -> bool {
    let take = (len as usize).min(HOLLOW_PREFIX_BYTES);
    if take == 0 {
        return true;
    }
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = vec![0u8; take];
    let mut read = 0usize;
    while read < take {
        match f.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(_) => return false,
        }
    }
    if read == 0 {
        return true;
    }
    buf[..read].iter().all(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn scratch(name: &str) -> std::path::PathBuf {
        let base =
            std::env::temp_dir().join(format!("rhino-open-fail-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("mkdir");
        base
    }

    #[test]
    fn empty_file_message() {
        let base = scratch("empty");
        let p = base.join("empty.avi");
        fs::File::create(&p).unwrap();
        assert_eq!(preflight_user_message(&p), Some(msg::EMPTY_OR_INCOMPLETE));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn hollow_prefix_message() {
        let base = scratch("zeros");
        let p = base.join("zeros.avi");
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(&vec![0u8; 128 * 1024]).unwrap();
        assert_eq!(preflight_user_message(&p), Some(msg::EMPTY_OR_INCOMPLETE));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn real_prefix_passes_preflight() {
        let base = scratch("riff");
        let p = base.join("riff.avi");
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(b"RIFF....AVI ").unwrap();
        f.write_all(&vec![1u8; 2048]).unwrap();
        assert_eq!(preflight_user_message(&p), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_message() {
        let p = Path::new("/tmp/rhino-surely-missing-open-fail-xyz.avi");
        assert_eq!(preflight_user_message(p), Some(msg::MISSING));
    }

    #[test]
    fn drop_continue_only_for_unplayable_notices() {
        assert!(should_drop_from_continue(msg::EMPTY_OR_INCOMPLETE));
        assert!(should_drop_from_continue(msg::UNREADABLE_MEDIA));
        assert!(should_drop_from_continue(msg::MISSING));
        assert!(!should_drop_from_continue(msg::GENERIC));
        assert!(!should_drop_from_continue(
            "Player busy (transport or load in progress)."
        ));
    }
}
