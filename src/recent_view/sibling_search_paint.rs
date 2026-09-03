// Neighbour-strip paint key: skip [fill_row] only when paths and stored progress match.
// Progress comes from CatalogMem::paint_key (no SQLite here).

use std::cell::RefCell;
use std::path::PathBuf;

pub(super) type HitsPaint = Option<Vec<(PathBuf, u64, u64)>>;

/// `false` when the strip already shows this paint key.
pub(super) fn take_if_new(
    painted: &RefCell<HitsPaint>,
    snap: Vec<(PathBuf, u64, u64)>,
) -> bool {
    if painted.borrow().as_ref() == Some(&snap) {
        return false;
    }
    *painted.borrow_mut() = Some(snap);
    true
}
