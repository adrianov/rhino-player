// Neighbour-search reactive input: debounce, background filter, lucky click.
// `#[path]` submodule of [sibling_search_state.rs].

use std::rc::{Rc, Weak};
use std::sync::Arc;

use gtk::prelude::{ButtonExt, EditableExt, WidgetExt};

use super::filter_hop::{install_filter_flush, note_filter_done};
use super::SiblingSearchState;

impl SiblingSearchState {
    pub(crate) fn bind_ctx(self: &Rc<Self>, ctx: Weak<crate::recent_view::RecentContext>) {
        *self.ctx.borrow_mut() = Some(ctx);
        install_filter_flush(self);
        let s = Rc::clone(self);
        self.entry.connect_changed(move |_| s.on_changed());
        let s2 = Rc::clone(self);
        self.wire_enter(move || s2.open_first_hit());
        let s3 = Rc::clone(self);
        self.entry.connect_stop_search(move |_| s3.clear_query());
        // Warm the session index off the UI thread (feature 33).
        let s4 = Rc::clone(self);
        s4.warm_catalog();
    }

    pub(crate) fn wire_lucky(self: &Rc<Self>, lucky: &gtk::Button) {
        let s = Rc::clone(self);
        lucky.connect_clicked(move |_| s.on_lucky());
    }

    fn on_lucky(self: &Rc<Self>) {
        self.cancel_settle();
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        crate::user_action_log::act("continue lucky");
        if !self.entry.text().is_empty() {
            self.mute_change.set(true);
            self.entry.set_text("");
            self.mute_change.set(false);
        }
        self.roll_lucky();
        self.refill_now();
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
        if self.mute_change.get() {
            return;
        }
        crate::glib_source_drop::drop_glib_source(&self.debounce);
        // Newer draft supersedes any in-flight filter / deferred paint.
        self.cancel_filter();
        self.cancel_settle();
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
        self.cancel_filter();
        self.cancel_settle();
        if self.entry.text().is_empty() {
            self.commit_and_refill(String::new());
            return;
        }
        self.entry.set_text("");
    }

    /// Load catalog on a worker so first keystrokes never wait on SQLite.
    fn warm_catalog(self: &Rc<Self>) {
        if self.catalog.ready() {
            return;
        }
        // Do not bump filter_gen — a warm must not cancel an in-flight typed filter.
        let gen = self.filter_gen.get();
        let inbox = Arc::clone(&self.filter_inbox);
        if let Err(e) = std::thread::Builder::new()
            .name("rhino-search-catalog".into())
            .spawn(move || {
                let boot = super::super::catalog_boot_from_db(crate::db::files_catalog_epoch());
                note_filter_done(
                    &inbox,
                    gen,
                    String::new(),
                    super::super::FilterOutcome {
                        hits: Vec::new(),
                        capped: false,
                        learned: Vec::new(),
                        missing: Vec::new(),
                        catalog: Some(boot),
                    },
                );
            })
        {
            eprintln!("[rhino] search: catalog warm spawn: {e}");
        }
    }

    pub(super) fn start_filter(self: &Rc<Self>, draft: String) {
        let gen = self.bump_filter_gen();
        self.filter_pending.set(true);
        let prepared = self.catalog.try_filter_job();
        self.spawn_filter(gen, draft, prepared);
    }

    fn spawn_filter(
        self: &Rc<Self>,
        gen: u64,
        draft: String,
        prepared: Option<super::super::FilterJob>,
    ) {
        let q = draft.to_lowercase();
        let inbox = Arc::clone(&self.filter_inbox);
        if let Err(e) = std::thread::Builder::new()
            .name("rhino-search-filter".into())
            .spawn(move || {
                let ((rows, progress, bad), catalog) = match prepared {
                    Some(job) => (job, None),
                    None => {
                        let (job, boot) = super::super::filter_job_from_db();
                        (job, Some(boot))
                    }
                };
                let mut outcome = super::super::filter_name_hits(&rows, &bad, &q, &progress);
                outcome.catalog = catalog;
                note_filter_done(&inbox, gen, draft, outcome);
            })
        {
            eprintln!("[rhino] search: filter spawn: {e}");
            self.filter_pending.set(false);
        }
    }

    fn bump_filter_gen(&self) -> u64 {
        let gen = self.filter_gen.get().wrapping_add(1);
        self.filter_gen.set(gen);
        gen
    }

    pub(super) fn on_filter_done(
        self: &Rc<Self>,
        gen: u64,
        draft: String,
        mut outcome: super::super::FilterOutcome,
    ) {
        let boot = outcome.catalog.take();
        if gen != self.filter_gen.get() {
            self.take_catalog_boot(boot);
            return;
        }
        self.filter_pending.set(false);
        self.take_catalog_boot(boot);
        // Catalog-only warm (empty draft): index ready; strip unchanged.
        if draft.is_empty() && self.query.borrow().is_empty() && !self.lucky.is_active() {
            return;
        }
        let open_first = self.open_first.get();
        self.open_first.set(false);
        self.apply_filter_outcome(draft, outcome);
        self.refill_now();
        if open_first {
            self.open_first_now();
        }
    }

    fn take_catalog_boot(&self, boot: Option<super::super::CatalogBoot>) {
        if let Some(boot) = boot {
            self.catalog.install(boot);
        }
    }
}
