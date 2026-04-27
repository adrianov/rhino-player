//! Undo bar: restore after **remove from list** or **move to trash** (see [crate::trash_xdg]).
//!
//! `history::record` is left to the app after `apply`; both branches fix disk + `db` + watch_later.

use std::path::{Path, PathBuf};

use crate::media_probe::{restore_list_remove_undo, ListRemoveUndo};
use crate::trash_xdg;

/// One item on the session undo **LIFO** (see `app` snackbar).
pub enum ContinueBarUndo {
    /// Only history + resume/DB; file stayed on disk.
    ListRemove(ListRemoveUndo),
    /// File is under `in_trash` in XDG [Trash] `files/`.
    Trash {
        snap: ListRemoveUndo,
        in_trash: PathBuf,
    },
}

impl ContinueBarUndo {
    pub fn target_path(&self) -> &Path {
        match self {
            ContinueBarUndo::ListRemove(s) | ContinueBarUndo::Trash { snap: s, .. } => {
                s.path.as_path()
            }
        }
    }
}

/// Restore the file and/or resume cache. On failure, the caller can push the token back.
pub fn apply(u: &ContinueBarUndo) -> Result<(), String> {
    match u {
        ContinueBarUndo::ListRemove(s) => {
            restore_list_remove_undo(s);
            Ok(())
        }
        ContinueBarUndo::Trash { snap, in_trash } => {
            trash_xdg::untrash_to_target(in_trash, &snap.path).map_err(|e| e.to_string())?;
            restore_list_remove_undo(snap);
            Ok(())
        }
    }
}
