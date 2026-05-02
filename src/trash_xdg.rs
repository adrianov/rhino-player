//! Platform trash helpers: session **Undo** restores via [untrash_to_target].
//! - **Linux:** [gio::File::trash] plus Freedesktop `Trash/files` lookup ([find_trash_files_stored_path]).
//! - **macOS:** Finder Trash via [`crate::trash_macos`] (`NSFileManager::trashItemAtURL`); [untrash_to_target]
//!   restores with `rename` from `in_trash` when the path is under `.Trash`/`.Trashes`.

use std::path::{Path, PathBuf};

#[cfg(not(target_os = "macos"))]
use gtk::gio;
#[cfg(not(target_os = "macos"))]
use gtk::gio::prelude::FileExt;
#[cfg(not(target_os = "macos"))]
use glib::GStr;

/// Moves [path] to the user's Trash (**Err** = move failed).
///
/// **Ok(Some(p))**: path inside Trash for Undo. **Ok(None)** (Linux only): trashed copy not found under
/// Freedesktop `Trash/files`.
pub fn trash_local_file_for_undo(path: &Path) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "macos")]
    {
        crate::trash_macos::move_to_trash_ns(path).map(Some)
    }
    #[cfg(not(target_os = "macos"))]
    {
        gio::File::for_path(path)
            .trash(gio::Cancellable::NONE)
            .map_err(|e| e.to_string())?;
        let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        Ok(find_trash_files_stored_path(&want, None))
    }
}

#[cfg(target_os = "macos")]
fn is_macos_trash_item(p: &Path) -> bool {
    p.ancestors().any(|a| {
        a.file_name().is_some_and(|n| n == ".Trash" || n == ".Trashes")
    })
}

fn rename_cross_fs_ok(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Freedesktop `Trash` dirs under `$XDG_DATA_HOME`/`~/.local/share`.
#[cfg(not(target_os = "macos"))]
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

/// Parses a `Path=`/`file:` fragment from `.trashinfo` on Linux only.
#[cfg(not(target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
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

/// Resolves the trashed **file** path after [gio::File::trash] so Undo can call [untrash_to_target].
#[cfg(not(target_os = "macos"))]
pub fn find_trash_files_stored_path(
    original_before_trash: &Path,
    _size_bytes: Option<u64>,
) -> Option<PathBuf> {
    let base = trash_base()?;
    let files_dir = base.join("files");
    let info_dir = base.join("info");
    let want =
        std::fs::canonicalize(original_before_trash).unwrap_or_else(|_| original_before_trash.to_path_buf());
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

/// Move a file from Trash back to [target] and remove the corresponding `.trashinfo` when present.
pub fn untrash_to_target(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }

    #[cfg(target_os = "macos")]
    {
        if is_macos_trash_item(in_trash) {
            rename_cross_fs_ok(in_trash, target)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "trash: path is not in macOS Trash",
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
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
        let name = in_trash.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "trash: no file name")
        })?;
        let mut inf = name.to_string_lossy().into_owned();
        inf.push_str(".trashinfo");
        let info = info_dir.join(&inf);
        rename_cross_fs_ok(in_trash, target)?;
        if info.is_file() {
            let _ = std::fs::remove_file(info);
        }
        Ok(())
    }
}
