// [SiblingSearchState] — neighbour-search query, index, and strip paint bookkeeping.
// Reactive typing (debounce / commit) lives in [sibling_search_input.rs].
// Loaded as `#[path]` from sibling_search so module-ABC stays off the scan hub.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use super::lucky::{lucky_hint, search_hint, LuckySession};
use super::{classify_openable, NeighbourEntry, CONTINUE_DISPLAY_MAX};

/// Settled filter delay after typing stops (feature 33).
pub(super) const TYPE_DEBOUNCE_MS: u64 = 200;

type CtxSlot = RefCell<Option<Weak<crate::recent_view::RecentContext>>>;

/// Query text, sibling-file index, and result bookkeeping for one window.
pub(crate) struct SiblingSearchState {
    /// Entry + hint row; hidden with the continue strip after IM teardown.
    shell: gtk::Box,
    pub(super) entry: gtk::SearchEntry,
    hint: gtk::Label,
    /// Committed filter (drives strip paint). Entry text is draft until debounce.
    pub(super) query: RefCell<String>,
    /// Session neighbour index (path + openability); filled once per window.
    index: RefCell<Vec<NeighbourEntry>>,
    scanned: Cell<bool>,
    /// I'm Feeling Lucky session (shown / seen / reserved next).
    lucky: LuckySession,
    last_hits: RefCell<Option<(usize, bool)>>,
    /// Last neighbour paint key (path + stored resume/duration); matching skips [fill_row].
    painted: RefCell<paint::HitsPaint>,
    pub(super) ctx: CtxSlot,
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
    /// Skip [on_changed] while Lucky clears the entry so the strip does not flash watch-later.
    pub(super) mute_change: Cell<bool>,
}

#[path = "sibling_search_input.rs"]
mod input;
#[path = "sibling_search_paint.rs"]
mod paint;

impl SiblingSearchState {
    pub(super) fn new(shell: gtk::Box, entry: gtk::SearchEntry, hint: gtk::Label) -> Rc<Self> {
        Rc::new(Self {
            shell,
            entry,
            hint,
            query: RefCell::new(String::new()),
            index: RefCell::default(),
            scanned: Cell::new(false),
            lucky: LuckySession::new(),
            last_hits: RefCell::new(None),
            painted: RefCell::new(None),
            ctx: RefCell::new(None),
            debounce: RefCell::new(None),
            mute_change: Cell::new(false),
        })
    }

    pub(crate) fn searching(&self) -> bool {
        !self.query.borrow().is_empty() || self.lucky.is_active()
    }

    pub(crate) fn typing_pending(&self) -> bool {
        self.debounce.borrow().is_some()
    }

    /// `false` when the strip already shows these neighbour paths at the same stored progress.
    pub(crate) fn begin_hits_paint(&self, paths: &[PathBuf]) -> bool {
        paint::take_if_new(&self.painted, paths)
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
        self.ensure_index();
        let index = self.index.borrow();
        if !q.is_empty() {
            return Some(super::capped_name_hits(&index, &q));
        }
        self.lucky.retarget(&index);
        Some((self.lucky.strip_hits(&index)?, false))
    }

    fn ensure_index(&self) {
        super::index_fill_once(&self.scanned, &self.index, super::build_neighbour_index);
    }

    pub(super) fn roll_lucky(&self) {
        self.ensure_index();
        self.lucky.roll(&self.index.borrow(), CONTINUE_DISPLAY_MAX);
        self.query.borrow_mut().clear();
        self.clear_hits_paint();
    }

    pub(crate) fn append_lucky_warm(&self, paths: &mut Vec<PathBuf>) {
        self.lucky.append_warm(paths, &self.index.borrow());
    }

    pub(super) fn drop_lucky(&self) {
        self.lucky.deactivate();
    }

    pub(crate) fn index_path_for(&self, path: &std::path::Path) -> std::path::PathBuf {
        self.listed(path).unwrap_or_else(|| path.to_path_buf())
    }

    fn listed(&self, path: &std::path::Path) -> Option<std::path::PathBuf> {
        self.index
            .borrow()
            .iter()
            .find(|e| crate::video_ext::paths_same_file(&e.path, path))
            .map(|e| e.path.clone())
    }

    fn set_openable(&self, path: &std::path::Path, openable: bool) -> bool {
        let mut index = self.index.borrow_mut();
        let Some(e) = index
            .iter_mut()
            .find(|e| crate::video_ext::paths_same_file(&e.path, path))
        else {
            return false;
        };
        e.openable = openable;
        true
    }

    /// Mark a path unopenable after trash / removal so the next filter skips it without FS I/O.
    pub(crate) fn note_path_removed(&self, path: &std::path::Path) {
        if !self.set_openable(path, false) && self.searching() {
            eprintln!("[rhino] search: trash miss (index) path={}", path.display());
        }
        if self.lucky.is_active() && !self.lucky.refill_slot(path, &self.index.borrow()) {
            eprintln!("[rhino] lucky: trash refill missed path={}", path.display());
        }
        self.clear_hits_paint();
    }

    pub(crate) fn lucky_cards_showing(&self) -> bool {
        self.lucky.is_active() && self.query.borrow().trim().is_empty()
    }

    pub(crate) fn hits_kind(&self) -> super::StripKind {
        if self.lucky_cards_showing() {
            super::StripKind::Lucky
        } else {
            super::StripKind::NeighbourHits
        }
    }

    /// Drop a lucky card and refill the slot; file and catalog stay. `false` when not lucky.
    pub(crate) fn dismiss_lucky_card(&self, path: &std::path::Path) -> bool {
        if !self.lucky_cards_showing() || !self.lucky.refill_slot(path, &self.index.borrow()) {
            return false;
        }
        self.clear_hits_paint();
        true
    }

    /// Re-run open preflight for one indexed path (e.g. undo trash restore).
    pub(crate) fn refresh_path_openability(&self, path: &std::path::Path) {
        if let Some(listed) = self.listed(path) {
            self.set_openable(&listed, classify_openable(&listed));
        }
        self.clear_hits_paint();
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
        super::set_search_browse_visible(&self.shell, &self.entry, visible);
    }

    /// Remember this row for [hide_continue_strip] (one window).
    pub(crate) fn bind_strip_hide(self: &Rc<Self>) {
        super::sibling_search_bind::bind_strip(self);
    }
}
