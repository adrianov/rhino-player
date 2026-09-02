// Bound continue-strip search: hide + card trash/remove API (feature 33).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use gtk::prelude::{IsA, WidgetExt};

use super::sibling_search_state::SiblingSearchState;

thread_local! {
    /// Bound from continue-strip wiring; [dismiss_search_for_playback] / [hide_continue_strip].
    static STRIP_SEARCH: RefCell<Option<Weak<SiblingSearchState>>> = const { RefCell::new(None) };
}

fn with_bound_search<R>(f: impl FnOnce(Rc<SiblingSearchState>) -> R) -> Option<R> {
    STRIP_SEARCH.with(|c| c.borrow().as_ref().and_then(Weak::upgrade).map(f))
}

pub(super) fn bind_strip(s: &Rc<SiblingSearchState>) {
    STRIP_SEARCH.with(|c| *c.borrow_mut() = Some(Rc::downgrade(s)));
}

/// Drop search IM and unmap the row while the continue strip may still be visible.
pub fn dismiss_search_for_playback() {
    with_bound_search(|s| s.sync_browse_visible(false));
}

/// Hide the continue strip for playback: dismiss neighbour-search first, then unmap the strip.
pub fn hide_continue_strip(recent: &impl IsA<gtk::Widget>) {
    dismiss_search_for_playback();
    recent.set_visible(false);
}

fn card_listing(path: &Path) -> PathBuf {
    with_bound_search(|s| s.index_path_for(path)).unwrap_or_else(|| path.to_path_buf())
}

/// Continue-card **Trash**: pin listing identity, move the file, forget catalog, refill lucky.
/// `None` if the path is not a file or the platform trash call failed.
pub fn card_trashed(path: &Path) -> Option<(crate::media_probe::ListRemoveUndo, Option<PathBuf>)> {
    if !path.is_file() {
        eprintln!(
            "[rhino] continue: trash skipped (not a file) path={}",
            path.display()
        );
        return None;
    }
    let snap = crate::media_probe::capture_list_remove_undo(path);
    let listing = card_listing(path);
    let loc = move_card_to_trash(path)?;
    crate::media_probe::remove_continue_entry(&snap.path);
    forget_trashed(&snap.path, &listing);
    Some((snap, loc))
}

fn forget_trashed(path: &Path, listing: &Path) {
    note_path_trashed(path);
    crate::db::forget_file(path);
    if listing != path {
        note_path_trashed(listing);
        crate::db::forget_file(listing);
    }
}

fn move_card_to_trash(path: &Path) -> Option<Option<PathBuf>> {
    match crate::trash_xdg::trash_local_file_for_undo(path) {
        Err(e) => {
            eprintln!("[rhino] move to trash (continue card): {e}");
            None
        }
        Ok(loc) => {
            if loc.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            Some(loc)
        }
    }
}

/// Continue-card **Remove**. `None` when lucky dismissed the pick (file and resume stay).
pub fn card_removed(path: &Path) -> Option<crate::media_probe::ListRemoveUndo> {
    if with_bound_search(|s| s.dismiss_lucky_card(path)).unwrap_or(false) {
        return None;
    }
    let snap = crate::media_probe::capture_list_remove_undo(path);
    crate::media_probe::remove_continue_entry(path);
    Some(snap)
}

/// Drop a trashed path from the catalog and the neighbour index (card or playing-file trash).
pub fn note_path_trashed(path: &Path) {
    crate::db::forget_file(path);
    with_bound_search(|s| s.note_path_removed(path));
}

/// Neighbour-search openability: reclassify after undo-trash restore.
pub fn search_note_restored(
    cell: &Rc<RefCell<Option<Rc<crate::recent_view::RecentContext>>>>,
    path: &Path,
) {
    if let Some(s) = cell.borrow().as_ref().and_then(|c| c.search.as_ref()) {
        s.refresh_path_openability(path);
    }
}
