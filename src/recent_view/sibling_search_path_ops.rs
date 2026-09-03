// Neighbour-search path / openability / lucky slot ops (feature 33).
// `#[path]` submodule of [sibling_search_state.rs].

use std::path::Path;
use std::rc::Rc;

use super::super::{classify_openable, FilterOutcome, CONTINUE_DISPLAY_MAX};
use super::SiblingSearchState;

impl SiblingSearchState {
    pub(super) fn roll_lucky(&self) {
        self.cancel_filter();
        self.hit_cache.borrow_mut().take();
        self.catalog.lucky_roll(&self.lucky, CONTINUE_DISPLAY_MAX);
        self.query.borrow_mut().clear();
        self.clear_hits_paint();
    }

    pub(super) fn drop_lucky(&self) {
        self.lucky.deactivate();
    }

    pub(crate) fn index_path_for(&self, path: &Path) -> std::path::PathBuf {
        self.listed(path).unwrap_or_else(|| path.to_path_buf())
    }

    fn listed(&self, path: &Path) -> Option<std::path::PathBuf> {
        self.catalog
            .index()
            .iter()
            .find(|e| crate::video_ext::paths_same_file(&e.path, path))
            .map(|e| e.path.clone())
    }

    fn set_openable(&self, path: &Path, openable: bool) -> bool {
        let mut index = self.catalog.index_mut();
        let Some(e) = index
            .iter_mut()
            .find(|e| crate::video_ext::paths_same_file(&e.path, path))
        else {
            return false;
        };
        e.set_openable(openable);
        true
    }

    /// Mark a path unopenable after trash / removal so the next filter skips it without FS I/O.
    pub(crate) fn note_path_removed(&self, path: &Path) {
        if !self.set_openable(path, false) && self.searching() {
            eprintln!("[rhino] search: trash miss (index) path={}", path.display());
        }
        if let Some(c) = self.hit_cache.borrow_mut().as_mut() {
            c.hits
                .retain(|p| !crate::video_ext::paths_same_file(p, path));
        }
        if self.lucky.is_active() && !self.catalog.lucky_refill(&self.lucky, path) {
            eprintln!("[rhino] lucky: trash refill missed path={}", path.display());
        }
        self.clear_hits_paint();
    }

    pub(crate) fn lucky_cards_showing(&self) -> bool {
        self.lucky.is_active() && self.query.borrow().trim().is_empty()
    }

    /// Drop a lucky card and refill the slot; file and catalog stay. `false` when not lucky.
    pub(crate) fn dismiss_lucky_card(&self, path: &Path) -> bool {
        if !self.lucky_cards_showing() || !self.catalog.lucky_refill(&self.lucky, path) {
            return false;
        }
        self.clear_hits_paint();
        true
    }

    /// Re-run open preflight for one indexed path (e.g. undo trash restore).
    pub(crate) fn refresh_path_openability(self: &Rc<Self>, path: &Path) {
        if let Some(listed) = self.listed(path) {
            self.set_openable(&listed, classify_openable(&listed));
        }
        self.hit_cache.borrow_mut().take();
        self.clear_hits_paint();
        let q = self.query.borrow().clone();
        if !q.trim().is_empty() {
            self.start_filter(q);
        }
    }

    pub(super) fn cancel_filter(&self) {
        self.filter_gen.set(self.filter_gen.get().wrapping_add(1));
        self.filter_pending.set(false);
        self.open_first.set(false);
    }

    pub(super) fn apply_filter_outcome(&self, draft: String, outcome: FilterOutcome) {
        for (p, open) in &outcome.learned {
            self.set_openable(p, *open);
        }
        for p in &outcome.missing {
            let _ = crate::media_probe::forget_missing(p);
            self.set_openable(p, false);
        }
        let q = draft.trim().to_lowercase();
        *self.hit_cache.borrow_mut() = Some(super::HitCache {
            q,
            hits: outcome.hits,
            capped: outcome.capped,
        });
        *self.query.borrow_mut() = draft;
        self.clear_hits_paint();
    }

    /// Remember this row for [hide_continue_strip] (one window).
    pub(crate) fn bind_strip_hide(self: &Rc<Self>) {
        super::super::sibling_search_bind::bind_strip(self);
    }
}
