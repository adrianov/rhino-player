// [SiblingSearchState] — neighbour-search query, index, debounce, and strip repaint.

const TYPE_DEBOUNCE_MS: u64 = 250;
const RESCAN_MIN_AGE_SECS: u64 = 2;

type CtxSlot = RefCell<Option<Weak<crate::recent_view::RecentContext>>>;

/// Query text, sibling-file index, and result bookkeeping for one window.
pub(crate) struct SiblingSearchState {
    entry: gtk::SearchEntry,
    hint: gtk::Label,
    /// Committed filter (drives strip paint). Entry text is draft until debounce.
    query: RefCell<String>,
    index: RefCell<Vec<PathBuf>>,
    scanned_at: RefCell<Option<Instant>>,
    last_hits: RefCell<Option<(usize, bool)>>,
    /// Last neighbour paths painted; identical commits skip [fill_row].
    painted: RefCell<Option<Vec<PathBuf>>>,
    ctx: CtxSlot,
    debounce: RefCell<Option<glib::SourceId>>,
}

impl SiblingSearchState {
    pub(super) fn new(entry: gtk::SearchEntry, hint: gtk::Label) -> Rc<Self> {
        Rc::new(Self {
            entry,
            hint,
            query: RefCell::new(String::new()),
            index: RefCell::default(),
            scanned_at: RefCell::new(None),
            last_hits: RefCell::new(None),
            painted: RefCell::new(None),
            ctx: RefCell::new(None),
            debounce: RefCell::new(None),
        })
    }

    pub(crate) fn searching(&self) -> bool {
        !self.query.borrow().is_empty()
    }

    pub(crate) fn typing_pending(&self) -> bool {
        self.debounce.borrow().is_some()
    }

    /// `false` when the strip already shows these neighbour paths.
    pub(crate) fn begin_hits_paint(&self, paths: &[PathBuf]) -> bool {
        if self.painted.borrow().as_ref().is_some_and(|p| p == paths) {
            return false;
        }
        *self.painted.borrow_mut() = Some(paths.to_vec());
        true
    }

    pub(crate) fn clear_hits_paint(&self) {
        self.painted.borrow_mut().take();
    }

    pub(crate) fn current_hits(&self) -> Option<Vec<PathBuf>> {
        let q = self.query.borrow().trim().to_lowercase();
        if q.is_empty() {
            *self.last_hits.borrow_mut() = None;
            return None;
        }
        let files = self.scanned_files();
        let (hits, capped) = take_capped(collect_hits(&files, &q));
        *self.last_hits.borrow_mut() = Some((hits.len(), capped));
        Some(hits)
    }

    fn scanned_files(&self) -> Vec<PathBuf> {
        self.refresh_index_if_stale();
        self.index.borrow().clone()
    }

    pub(crate) fn note_repaint(&self) {
        self.hint.set_text(&match (*self.last_hits.borrow()).filter(|_| self.searching()) {
            None => String::new(),
            Some((n, true)) => format!("{n}+ matches"),
            Some((0, false)) => "No matches".to_string(),
            Some((n, false)) => format!("{n} match{}", if n == 1 { "" } else { "es" }),
        });
    }

    pub(crate) fn bind_ctx(self: &Rc<Self>, ctx: Weak<crate::recent_view::RecentContext>) {
        *self.ctx.borrow_mut() = Some(ctx);
        let s = Rc::clone(self);
        self.entry.connect_changed(move |_| s.on_changed());
        let s2 = Rc::clone(self);
        self.wire_enter(move || s2.open_first_hit());
        let s3 = Rc::clone(self);
        self.entry.connect_stop_search(move |_| s3.clear_query());
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
            std::time::Duration::from_millis(TYPE_DEBOUNCE_MS),
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

    fn refresh_index_if_stale(&self) {
        if self.index_fresh() {
            return;
        }
        *self.index.borrow_mut() = scan_watch_later_dirs();
        *self.scanned_at.borrow_mut() = Some(Instant::now());
    }

    fn index_fresh(&self) -> bool {
        self.scanned_at.borrow().is_some_and(|t| {
            t.elapsed() < std::time::Duration::from_secs(RESCAN_MIN_AGE_SECS)
        }) && !self.index.borrow().is_empty()
    }
}

fn take_capped(mut hits: Vec<PathBuf>) -> (Vec<PathBuf>, bool) {
    let capped = hits.len() > SEARCH_MAX_HITS;
    hits.truncate(SEARCH_MAX_HITS);
    (hits, capped)
}
