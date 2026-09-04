use std::path::{Path, PathBuf};

use glib::GStr;
use gtk::gio;

/// Parses a `Path=`/`file:` fragment from `.trashinfo`.
fn local_path_from_trashinfo_value(v: &str) -> Option<PathBuf> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with("file:") {
        return gio::File::for_uri(t).path();
    }
    let dec = glib::uri_unescape_string(t, GStr::NONE)?;
    let s = dec.to_string();
    if s.starts_with('/') {
        return Some(PathBuf::from(s));
    }
    None
}

/// Value after the first `Path=` in trashinfo.
fn path_value_from_info(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Path=") {
            return Some(rest.to_string());
        }
        if let Some(rest) = t.strip_prefix("path=") {
            return Some(rest.to_string());
        }
    }
    None
}

/// Outcome of inspecting one `Trash/info` entry against the wanted original path.
enum InfoScan {
    /// Entry matches: stored file path plus info mtime.
    Match(PathBuf, std::time::SystemTime),
    Skip,
    /// Unreadable or unparseable `.trashinfo`: historical `?` semantics abort the whole scan.
    Abort,
}

/// Records a matching entry into [best] when newer; [InfoScan::Abort] yields [None], which the
/// caller propagates out of the whole scan.
fn keep_newer(best: &mut Option<(PathBuf, std::time::SystemTime)>, scan: InfoScan) -> Option<()> {
    match scan {
        InfoScan::Abort => None,
        InfoScan::Skip => Some(()),
        InfoScan::Match(in_files, t) => {
            if best.as_ref().map_or(true, |(_, tt)| t > *tt) {
                *best = Some((in_files, t));
            }
            Some(())
        }
    }
}

/// Classifies one `Trash/info` directory entry.
fn scan_trashinfo_entry(e: &std::fs::DirEntry, files_dir: &Path, want: &Path) -> InfoScan {
    let ip = e.path();
    if ip.extension() != Some(std::ffi::OsStr::new("trashinfo")) {
        return InfoScan::Skip;
    }
    // Unreadable or unparseable `.trashinfo`: historical `?` semantics abort the whole scan.
    let want_orig = match trashinfo_original_path(&ip) {
        Some(p) => p,
        None => return InfoScan::Abort,
    };
    if want_orig != want {
        return InfoScan::Skip;
    }
    stored_file_for_info(e, &ip, files_dir)
}

/// Recorded original path of a `.trashinfo`, or [None] when unreadable/unparseable/absent.
fn trashinfo_original_path(ip: &Path) -> Option<PathBuf> {
    let s = std::fs::read_to_string(ip).ok()?;
    local_path_from_trashinfo_value(&path_value_from_info(&s)?)
}

/// Stored path plus info mtime for an entry whose recorded original matched; missing file skips.
fn stored_file_for_info(e: &std::fs::DirEntry, ip: &Path, files_dir: &Path) -> InfoScan {
    let stem = match ip.file_stem() {
        Some(stem) => stem.to_owned(),
        None => return InfoScan::Abort,
    };
    let in_files = files_dir.join(stem);
    if !in_files.is_file() {
        return InfoScan::Skip;
    }
    InfoScan::Match(
        in_files,
        e.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::UNIX_EPOCH),
    )
}
