// Neighbour-search commit / Enter: cache reuse, clear, open-first (feature 33).
// `#[path]` submodule of [sibling_search_state.rs].

use std::rc::Rc;

use gtk::prelude::EditableExt;

use super::SiblingSearchState;

impl SiblingSearchState {
    pub(super) fn commit_and_refill(self: &Rc<Self>, next: String) {
        if next.is_empty() {
            self.clear_committed_query();
            return;
        }
        if self.reuse_hit_cache(&next) {
            return;
        }
        self.drop_lucky();
        self.start_filter(next);
    }

    fn clear_committed_query(self: &Rc<Self>) {
        self.cancel_filter();
        self.drop_lucky();
        self.hit_cache.borrow_mut().take();
        self.query.borrow_mut().clear();
        self.refill_now();
    }

    pub(super) fn reuse_hit_cache(self: &Rc<Self>, next: &str) -> bool {
        let q = next.to_lowercase();
        if !self
            .hit_cache
            .borrow()
            .as_ref()
            .is_some_and(|c| c.q == q)
            || self.lucky.is_active()
        {
            return false;
        }
        *self.query.borrow_mut() = next.to_string();
        self.refill_now();
        true
    }

    pub(super) fn open_first_hit(self: &Rc<Self>) {
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        let draft = Self::draft_text(&self.entry);
        if draft.is_empty() {
            self.open_first_now();
            return;
        }
        if self.reuse_hit_cache(&draft) {
            self.open_first_now();
            return;
        }
        self.open_first.set(true);
        if self.filter_pending.get() {
            return;
        }
        self.drop_lucky();
        self.start_filter(draft);
    }

    pub(super) fn open_first_now(&self) {
        let Some(first) = self.current_hits().and_then(|h| h.into_iter().next()) else {
            return;
        };
        if let Some(c) = self.ctx.borrow().as_ref().and_then(|w| w.upgrade()) {
            c.open_path(&first);
        }
    }

    pub(super) fn draft_text(entry: &gtk::SearchEntry) -> String {
        entry.text().trim().to_string()
    }

    /// Queue strip paint on idle so keystrokes stay ahead of GTK rebuild.
    pub(super) fn refill_now(self: &Rc<Self>) {
        self.settle_pending.set(true);
        crate::glib_source_drop::drop_glib_source(&self.paint_idle);
        let s = Rc::clone(self);
        *self.paint_idle.borrow_mut() = Some(glib::idle_add_local_once(move || {
            crate::glib_source_drop::finish_glib_source(&s.paint_idle);
            s.run_settled_paint();
        }));
    }

    fn run_settled_paint(&self) {
        self.settle_pending.set(false);
        if self.debounce.borrow().is_some() || self.filter_pending.get() {
            return;
        }
        if let Some(c) = self.ctx.borrow().as_ref().and_then(|w| w.upgrade()) {
            c.apply_strip();
        }
        self.note_repaint();
    }

    pub(super) fn cancel_settle(&self) {
        crate::glib_source_drop::drop_glib_source(&self.paint_idle);
        self.settle_pending.set(false);
    }
}
