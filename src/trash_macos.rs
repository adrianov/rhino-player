//! Trash / restore via **`NSFileManager`** (same as Finder), for GTK-on-macOS builds where
//! [`gio::File::trash`] is unreliable.

use objc2::rc::Retained;
use objc2_foundation::{NSFileManager, NSURL};
use std::path::{Path, PathBuf};

/// Move file at [path] into the user Trash; returns filesystem path Finder used for Undo.
///
/// Canonicalizes relative paths before building [`NSURL`] (required by Foundation).
pub fn move_to_trash_ns(path: &Path) -> Result<PathBuf, String> {
    let abs = std::fs::canonicalize(path).map_err(|e| format!("trash: {e}"))?;
    let url =
        NSURL::from_file_path(&abs).ok_or_else(|| "trash: path not representable".to_string())?;
    let fm = NSFileManager::defaultManager();
    let mut out: Option<Retained<NSURL>> = None;
    fm.trashItemAtURL_resultingItemURL_error(&url, Some(&mut out))
        .map_err(|e| e.localizedDescription().to_string())?;
    let trashed = out.ok_or_else(|| "trash: no resulting trash URL".to_string())?;
    trashed
        .to_file_path()
        .ok_or_else(|| "trash: could not read trashed file path".to_string())
}
