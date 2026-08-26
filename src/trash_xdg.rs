//! Platform trash helpers: session **Undo** restores via [untrash_to_target].
//! - **Linux:** [gio::File::trash] plus Freedesktop `Trash/files` lookup ([find_trash_files_stored_path]).
//! - **macOS:** Finder Trash via [`crate::trash_macos`] (`NSFileManager::trashItemAtURL`); [untrash_to_target]
//!   restores with `rename` from `in_trash` when the path is under `.Trash`/`.Trashes`.

#[cfg(not(target_os = "macos"))]
mod linux;
use std::path::{Path, PathBuf};

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
        linux::trash_via_gio(path)
    }
}

#[cfg(target_os = "macos")]
fn is_macos_trash_item(p: &Path) -> bool {
    p.ancestors().any(|a| {
        a.file_name()
            .is_some_and(|n| n == ".Trash" || n == ".Trashes")
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

/// Move a file from Trash back to [target] and remove the corresponding `.trashinfo` when present.
pub fn untrash_to_target(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }

    #[cfg(target_os = "macos")]
    {
        untrash_from_macos_trash(in_trash, target)
    }

    #[cfg(not(target_os = "macos"))]
    {
        linux::untrash_from_xdg_trash(in_trash, target)
    }
}

#[cfg(target_os = "macos")]
fn untrash_from_macos_trash(in_trash: &Path, target: &Path) -> std::io::Result<()> {
    if is_macos_trash_item(in_trash) {
        rename_cross_fs_ok(in_trash, target)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trash: path is not in macOS Trash",
        ))
    }
}
