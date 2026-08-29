// Neighbour-search reactive input: debounce, commit, Enter / Escape clear.
// `#[path]` submodule of [sibling_search_state.rs].

use std::rc::{Rc, Weak};

use gtk::prelude::{EditableExt, WidgetExt};

use super::SiblingSearchState;

impl SiblingSearchState {
    pub(crate) fn bind_ctx(self: &Rc<Self>, ctx: Weak<crate::recent_view::RecentContext>) {
        *self.ctx.borrow_mut() = Some(ctx);
        let s = Rc::clone(self);
        self.entry.connect_changed(move |_| s.on_changed());
        let s2 = Rc::clone(self);
        self.wire_enter(move || s2.open_first_hit());
        let s3 = Rc::clone(self);
        self.entry.connect_stop_search(move |_| s3.clear_query());
        // One idle: build the session index before the user finishes typing (feature 33).
        let s4 = Rc::clone(self);
        glib::idle_add_local_once(move || {
            let _ = s4.neighbour_index();
        });
    }

    fn wire_enter(&self, act: impl Fn() + 'static) {
        let k = gtk::EventControllerKey::new();
        k.connect_key_pressed(move |_, key, _, _| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
                act();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.entry.add_controller(k);
    }

    fn on_changed(self: &Rc<Self>) {
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        if Self::draft_text(&self.entry).is_empty() {
            self.commit_and_refill(String::new());
            return;
        }
        self.arm_debounce();
    }

    fn arm_debounce(self: &Rc<Self>) {
        let s = Rc::clone(self);
        *self.debounce.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(super::TYPE_DEBOUNCE_MS),
            move || {
                crate::glib_source_drop::finish_glib_source(&s.debounce);
                s.commit_and_refill(Self::draft_text(&s.entry));
            },
        ));
    }

    fn clear_query(self: &Rc<Self>) {
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        if self.entry.text().is_empty() {
            self.commit_and_refill(String::new());
            return;
        }
        self.entry.set_text("");
    }

    fn draft_text(entry: &gtk::SearchEntry) -> String {
        entry.text().trim().to_string()
    }

    fn commit_and_refill(&self, next: String) {
        if *self.query.borrow() == next {
            self.note_repaint();
            return;
        }
        *self.query.borrow_mut() = next;
        self.refill_now();
    }

    fn refill_now(&self) {
        if let Some(c) = self.ctx.borrow().as_ref().and_then(|w| w.upgrade()) {
            c.apply_strip();
        }
        self.note_repaint();
    }

    fn open_first_hit(self: &Rc<Self>) {
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        self.commit_and_refill(Self::draft_text(&self.entry));
        if !self.searching() {
            return;
        }
        let Some(first) = self.current_hits().and_then(|h| h.into_iter().next()) else {
            return;
        };
        if let Some(c) = self.ctx.borrow().as_ref().and_then(|w| w.upgrade()) {
            c.open_path(&first);
        }
    }
}
