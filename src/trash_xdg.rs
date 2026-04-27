//! Freedesktop trash ([`$XDG_DATA_HOME/Trash`]) to locate a trashed file and restore it. See
//! [Trash spec](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html).

use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::gio::prelude::FileExt;

use glib::GStr;

/// `~/.local/share` (or `XDG_DATA_HOME`) with `…/Trash`.
fn trash_base() -> Option<PathBuf> {
    let d = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            let h = std::env::var_os("HOME")?;
            Some(PathBuf::from(h).join(".local/share"))
        })?;
    let b = d.join("Trash");
    let files = b.join("files");
    let info = b.join("info");
    if !files.is_dir() || !info.is_dir() {
        return None;
    }
    Some(b)
}

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

/// Original path the item was at before it was trashed. Compare with
/// [std::fs::canonicalize] from **before** [gio::File::trash] when still on disk.
pub fn find_trash_files_stored_path(original: &Path) -> Option<PathBuf> {
    let base = trash_base()?;
    let files_dir = base.join("files");
    let info_dir = base.join("info");
    let want = std::fs::canonicalize(original).unwrap_or_else(|_| original.to_path_buf());
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    let entries = std::fs::read_dir(&info_dir).ok()?;
    for e in entries.filter_map(std::io::Result::ok) {
        let ip = e.path();
        if ip.extension() != Some(std::ffi::OsStr::new("trashinfo")) {
            continue;
        }
        let s = std::fs::read_to_string(&ip).ok()?;
        let raw = path_value_from_info(&s)?;
        let p = local_path_from_trashinfo_value(&raw)?;
        if p != want {
            continue;
        }
        let stem = ip.file_stem()?;
        let in_files = files_dir.join(stem);
        if !in_files.is_file() {
            continue;
        }
        let t = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::UNIX_EPOCH);
        let take = best.as_ref().map_or(true, |(_, tt)| t > *tt);
        if take {
            best = Some((in_files, t));
        }
    }
    best.map(|(p, _)| p)
}

/// Move a file from [trash] `files/…` back to [target] and remove the corresponding `.trashinfo`.
pub fn untrash_to_target(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    let (files_dir, info_dir) = {
        let b = trash_base()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no XDG trash"))?;
        (b.join("files"), b.join("info"))
    };
    if in_trash.parent() != Some(files_dir.as_path()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trash: path is not in Trash/files",
        ));
    }
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }
    let name = in_trash.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "trash: no file name")
    })?;
    let mut inf = name.to_string_lossy().into_owned();
    inf.push_str(".trashinfo");
    let info = info_dir.join(&inf);
    std::fs::rename(in_trash, target)?;
    if info.is_file() {
        let _ = std::fs::remove_file(info);
    }
    Ok(())
}
