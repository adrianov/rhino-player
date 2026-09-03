// [SiblingSearchState] — neighbour-search query and strip bookkeeping.
// CatalogMem owns SQLite snap + Lucky/search coordination (feature 33).
// Reactive typing / filter worker: [sibling_search_input.rs]; paint key: [sibling_search_paint.rs].

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

#[path = "sibling_search_filter_hop.rs"]
mod filter_hop;
#[path = "sibling_search_commit.rs"]
mod commit;
#[path = "sibling_search_input.rs"]
mod input;
#[path = "sibling_search_path_ops.rs"]
mod path_ops;
#[path = "sibling_search_paint.rs"]
mod paint;

use super::lucky::{lucky_hint, search_hint, LuckySession};
use super::{CatalogMem, StripKind};
use filter_hop::FilterInbox;

/// Settled filter delay after typing stops (feature 33).
pub(super) const TYPE_DEBOUNCE_MS: u64 = 200;

type CtxSlot = RefCell<Option<Weak<crate::recent_view::RecentContext>>>;

/// Settled name-search hits (paint reads this; workers fill it).
pub(super) struct HitCache {
    pub(super) q: String,
    pub(super) hits: Vec<PathBuf>,
    pub(super) capped: bool,
}

/// Query text, sibling-file index, and result bookkeeping for one window.
pub(crate) struct SiblingSearchState {
    /// Entry + hint row; hidden with the continue strip after IM teardown.
    shell: gtk::Box,
    pub(super) entry: gtk::SearchEntry,
    hint: gtk::Label,
    /// Committed filter (drives strip paint). Entry text is draft until debounce + worker.
    pub(super) query: RefCell<String>,
    /// In-memory catalog + progress (one SQLite load per window).
    catalog: CatalogMem,
    /// I'm Feeling Lucky session (shown / seen / reserved next).
    lucky: LuckySession,
    last_hits: RefCell<Option<(usize, bool)>>,
    /// Last neighbour paint key; matching skips [fill_row].
    painted: RefCell<paint::HitsPaint>,
    /// Cached name-search hits for the committed query (no re-score on paint).
    hit_cache: RefCell<Option<HitCache>>,
    /// Bumped when a newer filter supersedes an in-flight worker.
    filter_gen: Cell<u64>,
    /// True while a name-filter worker has not yet applied.
    filter_pending: Cell<bool>,
    /// True from settled commit until the deferred strip paint runs.
    settle_pending: Cell<bool>,
    /// Open the first hit when the in-flight filter lands (Enter while filtering).
    open_first: Cell<bool>,
    /// Worker → main filter result inbox.
    filter_inbox: Arc<FilterInbox>,
    pub(super) ctx: CtxSlot,
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
    /// Deferred strip paint after filter / clear (yields to keystrokes).
    pub(super) paint_idle: RefCell<Option<glib::SourceId>>,
    /// Skip [on_changed] while Lucky clears the entry so the strip does not flash watch-later.
    pub(super) mute_change: Cell<bool>,
}

impl SiblingSearchState {
    pub(super) fn new(shell: gtk::Box, entry: gtk::SearchEntry, hint: gtk::Label) -> Rc<Self> {
        Rc::new(Self {
            shell,
            entry,
            hint,
            query: RefCell::new(String::new()),
            catalog: CatalogMem::new(),
            lucky: LuckySession::new(),
            last_hits: RefCell::new(None),
            painted: RefCell::new(None),
            hit_cache: RefCell::new(None),
            filter_gen: Cell::new(0),
            filter_pending: Cell::new(false),
            settle_pending: Cell::new(false),
            open_first: Cell::new(false),
            filter_inbox: FilterInbox::new(),
            ctx: RefCell::new(None),
            debounce: RefCell::new(None),
            paint_idle: RefCell::new(None),
            mute_change: Cell::new(false),
        })
    }

    pub(crate) fn searching(&self) -> bool {
        !self.query.borrow().is_empty() || self.lucky.is_active()
    }

    pub(crate) fn typing_pending(&self) -> bool {
        self.debounce.borrow().is_some()
            || self.filter_pending.get()
            || self.settle_pending.get()
    }

    /// Memory-only cards for a settled search / Lucky strip (no disk or SQLite).
    pub(crate) fn strip_cards(&self, paths: &[PathBuf]) -> Vec<crate::media_probe::CardData> {
        self.catalog.strip_cards(paths)
    }

    /// `false` when the strip already shows these neighbour paths at the same stored progress.
    pub(crate) fn begin_hits_paint(&self, paths: &[PathBuf]) -> bool {
        paint::take_if_new(&self.painted, self.catalog.paint_key(paths))
    }

    pub(crate) fn clear_hits_paint(&self) {
        self.painted.borrow_mut().take();
    }

    pub(crate) fn current_hits(&self) -> Option<Vec<PathBuf>> {
        let hits = self.hits_for_strip();
        *self.last_hits.borrow_mut() = hits.as_ref().map(|(h, c)| (h.len(), *c));
        hits.map(|(h, _)| h)
    }

    fn hits_for_strip(&self) -> Option<(Vec<PathBuf>, bool)> {
        let q = self.query.borrow().trim().to_lowercase();
        if !q.is_empty() {
            let cache = self.hit_cache.borrow();
            let c = cache.as_ref().filter(|c| c.q == q)?;
            return Some((c.hits.clone(), c.capped));
        }
        self.ensure_index();
        Some((self.catalog.lucky_hits(&self.lucky)?, false))
    }

    pub(super) fn ensure_index(&self) {
        self.catalog.ensure();
    }

    pub(crate) fn hits_kind(&self) -> StripKind {
        if self.lucky_cards_showing() {
            StripKind::Lucky
        } else {
            StripKind::NeighbourHits
        }
    }

    pub(crate) fn note_repaint(&self) {
        self.hint.set_text(&self.hint_text());
    }

    fn hint_text(&self) -> String {
        let Some((n, capped)) = *self.last_hits.borrow() else {
            return String::new();
        };
        if !self.searching() {
            return String::new();
        }
        if self.lucky_cards_showing() {
            return lucky_hint(n);
        }
        search_hint(n, capped)
    }

    /// Browse strip shown ↔ search row mapped. Hide path drops IM then unmaps the row so
    /// IBus / gdk-macos cannot leave a status mark over the video.
    pub(crate) fn sync_browse_visible(&self, visible: bool) {
        if visible && self.catalog.ready() {
            self.catalog.refresh_progress();
        }
        super::set_search_browse_visible(&self.shell, &self.entry, visible);
    }
}
