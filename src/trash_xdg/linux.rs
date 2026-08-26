//! Linux/XDG Freedesktop trash: `gio` trashing, `Trash/files` lookup and restore.

use std::path::{Path, PathBuf};

use glib::GStr;
use gtk::gio;
use gtk::gio::prelude::FileExt;

/// Moves [path] to Trash via [gio::File::trash] and looks up the stored copy for Undo.
pub(super) fn trash_via_gio(path: &Path) -> Result<Option<PathBuf>, String> {
    gio::File::for_path(path)
        .trash(gio::Cancellable::NONE)
        .map_err(|e| e.to_string())?;
    let want = canonical_or_self(path);
    Ok(find_trash_files_stored_path(&want, None))
}

/// Canonicalized [path], or the path unchanged when canonicalization fails.
fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Freedesktop `Trash` dirs under `$XDG_DATA_HOME`/`~/.local/share`.
fn trash_base() -> Option<PathBuf> {
    let b = xdg_data_home()?.join("Trash");
    let files = b.join("files");
    let info = b.join("info");
    if !files.is_dir() || !info.is_dir() {
        return None;
    }
    Some(b)
}

/// `$XDG_DATA_HOME` when absolute, else `~/.local/share`.
fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            let h = std::env::var_os("HOME")?;
            Some(PathBuf::from(h).join(".local/share"))
        })
}

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

/// Resolves the trashed **file** path after [gio::File::trash] so Undo can call
/// [super::untrash_to_target].
pub(super) fn find_trash_files_stored_path(
    original_before_trash: &Path,
    _size_bytes: Option<u64>,
) -> Option<PathBuf> {
    let base = trash_base()?;
    let files_dir = base.join("files");
    let info_dir = base.join("info");
    let want = canonical_or_self(original_before_trash);
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for e in std::fs::read_dir(&info_dir)
        .ok()?
        .filter_map(std::io::Result::ok)
    {
        keep_newer(
            &mut best,
            scan_trashinfo_entry(&e, files_dir.as_path(), &want),
        )?;
    }
    best.map(|(p, _)| p)
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
    let t = e
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(std::time::UNIX_EPOCH);
    InfoScan::Match(in_files, t)
}

/// Restores via `rename` from `Trash/files`, removing the matching `.trashinfo`.
pub(super) fn untrash_from_xdg_trash(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    let (files_dir, info_dir) = xdg_trash_dirs()?;
    if in_trash.parent() != Some(files_dir.as_path()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trash: path is not in Trash/files",
        ));
    }
    let info = sibling_trashinfo(in_trash, &info_dir)?;
    super::rename_cross_fs_ok(in_trash, target)?;
    if info.is_file() {
        let _ = std::fs::remove_file(info);
    }
    Ok(())
}

/// `.trashinfo` path named after [in_trash].
fn sibling_trashinfo(in_trash: &Path, info_dir: &Path) -> std::io::Result<PathBuf> {
    let name = in_trash.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "trash: no file name")
    })?;
    let mut inf = name.to_string_lossy().into_owned();
    inf.push_str(".trashinfo");
    Ok(info_dir.join(&inf))
}

/// `Trash/files` and `Trash/info` under [trash_base].
fn xdg_trash_dirs() -> std::io::Result<(PathBuf, PathBuf)> {
    let b = trash_base()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no XDG trash"))?;
    Ok((b.join("files"), b.join("info")))
}
