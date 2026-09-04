//! Linux/XDG Freedesktop trash: `gio` trashing, `Trash/files` lookup and restore.

use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::gio::prelude::FileExt;

include!("linux_trashinfo.rs");
include!("linux_scan.rs");

/// Moves [path] to Trash via [gio::File::trash] and looks up the stored copy for Undo.
pub(super) fn trash_via_gio(path: &Path) -> Result<Option<PathBuf>, String> {
    gio::File::for_path(path)
        .trash(gio::Cancellable::NONE)
        .map_err(|e| e.to_string())?;
    Ok(find_trash_files_stored_path(&canonical_or_self(path), None))
}

/// Canonicalized [path], or the path unchanged when canonicalization fails.
fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Freedesktop `Trash` dirs under `$XDG_DATA_HOME`/`~/.local/share`.
fn trash_base() -> Option<PathBuf> {
    let b = xdg_data_home()?.join("Trash");
    if !b.join("files").is_dir() || !b.join("info").is_dir() {
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
