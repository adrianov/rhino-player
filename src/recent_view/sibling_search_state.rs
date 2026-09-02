// [SiblingSearchState] — neighbour-search query, index, and strip paint bookkeeping.
// Reactive typing (debounce / commit) lives in [sibling_search_input.rs].
// Loaded as `#[path]` from sibling_search so module-ABC stays off the scan hub.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::glib::prelude::CastNone;
use gtk::prelude::{GtkWindowExt, IsA, WidgetExt};

use super::sibling_search_lucky::{keep_openable, lucky_hint, lucky_picks, search_hint};
use super::{
    build_neighbour_index, classify_openable, index_fill_once, present_name_hits, take_capped,
    NeighbourEntry, CONTINUE_DISPLAY_MAX,
};

/// Settled filter delay after typing stops (feature 33).
pub(super) const TYPE_DEBOUNCE_MS: u64 = 200;

type CtxSlot = RefCell<Option<Weak<crate::recent_view::RecentContext>>>;

thread_local! {
    /// Bound from continue-strip wiring; [dismiss_search_for_playback] / [hide_continue_strip].
    static STRIP_SEARCH: RefCell<Option<Weak<SiblingSearchState>>> = const { RefCell::new(None) };
}

/// Drop search focus and unmap the row while the continue strip may still be visible.
pub fn dismiss_search_for_playback() {
    STRIP_SEARCH.with(|c| {
        if let Some(s) = c.borrow().as_ref().and_then(Weak::upgrade) {
            s.sync_browse_visible(false);
        }
    });
}

/// Hide the continue strip for playback: dismiss neighbour-search first, then unmap the strip.
pub fn hide_continue_strip(recent: &impl IsA<gtk::Widget>) {
    dismiss_search_for_playback();
    recent.set_visible(false);
}

/// Query text, sibling-file index, and result bookkeeping for one window.
pub(crate) struct SiblingSearchState {
    /// Entry + hint row; hidden with the continue strip so gdk-macos tears down IM.
    shell: gtk::Box,
    pub(super) entry: gtk::SearchEntry,
    hint: gtk::Label,
    /// Committed filter (drives strip paint). Entry text is draft until debounce.
    pub(super) query: RefCell<String>,
    /// Session neighbour index (path + openability); filled once per window.
    index: RefCell<Vec<NeighbourEntry>>,
    scanned: Cell<bool>,
    /// I'm Feeling Lucky handful; takes the strip when the query is empty.
    lucky: RefCell<Option<Vec<PathBuf>>>,
    last_hits: RefCell<Option<(usize, bool)>>,
    /// Last neighbour paths painted; identical commits skip [fill_row].
    painted: RefCell<Option<Vec<PathBuf>>>,
    pub(super) ctx: CtxSlot,
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
    /// Skip [on_changed] while Lucky clears the entry so the strip does not flash watch-later.
    pub(super) mute_change: Cell<bool>,
}

#[path = "sibling_search_input.rs"]
mod input;

impl SiblingSearchState {
    pub(super) fn new(shell: gtk::Box, entry: gtk::SearchEntry, hint: gtk::Label) -> Rc<Self> {
        Rc::new(Self {
            shell,
            entry,
            hint,
            query: RefCell::new(String::new()),
            index: RefCell::default(),
            scanned: Cell::new(false),
            lucky: RefCell::new(None),
            last_hits: RefCell::new(None),
            painted: RefCell::new(None),
            ctx: RefCell::new(None),
            debounce: RefCell::new(None),
            mute_change: Cell::new(false),
        })
    }

    pub(crate) fn searching(&self) -> bool {
        !self.query.borrow().is_empty() || self.lucky.borrow().is_some()
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
        let hits = self.hits_for_strip();
        *self.last_hits.borrow_mut() = hits.as_ref().map(|(h, c)| (h.len(), *c));
        hits.map(|(h, _)| h)
    }

    fn hits_for_strip(&self) -> Option<(Vec<PathBuf>, bool)> {
        let q = self.query.borrow().trim().to_lowercase();
        if !q.is_empty() {
            return Some(take_capped(present_name_hits(&self.neighbour_index(), &q)));
        }
        let lucky = self.lucky.borrow();
        Some((keep_openable(lucky.as_ref()?, &self.index.borrow()), false))
    }

    pub(super) fn roll_lucky(&self) {
        let picks = lucky_picks(&self.neighbour_index(), CONTINUE_DISPLAY_MAX);
        *self.lucky.borrow_mut() = Some(picks);
        self.query.borrow_mut().clear();
        self.clear_hits_paint();
    }

    pub(super) fn drop_lucky(&self) {
        self.lucky.borrow_mut().take();
    }

    pub(super) fn neighbour_index(&self) -> Vec<NeighbourEntry> {
        index_fill_once(&self.scanned, &self.index, build_neighbour_index)
    }

    /// Mark a path unopenable after trash / removal so the next filter skips it without FS I/O.
    pub(crate) fn note_path_removed(&self, path: &std::path::Path) {
        if let Some(e) = self.index.borrow_mut().iter_mut().find(|e| e.path == path) {
            e.openable = false;
        }
        self.clear_hits_paint();
    }

    /// Re-run open preflight for one indexed path (e.g. undo trash restore).
    pub(crate) fn refresh_path_openability(&self, path: &std::path::Path) {
        if let Some(e) = self.index.borrow_mut().iter_mut().find(|e| e.path == path) {
            e.openable = classify_openable(&e.path);
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
        if self.lucky.borrow().is_some() && self.query.borrow().is_empty() {
            return lucky_hint(n);
        }
        search_hint(n, capped)
    }

    /// Browse strip shown ↔ search row mapped. Hide path drops focus then unmaps the row so
    /// gdk-macos cannot leave an IM badge over the video.
    pub(crate) fn sync_browse_visible(&self, visible: bool) {
        if !visible {
            self.drop_window_focus();
        }
        self.shell.set_visible(visible);
    }

    /// Remember this row for [hide_continue_strip] (one window).
    pub(crate) fn bind_strip_hide(self: &Rc<Self>) {
        STRIP_SEARCH.with(|c| *c.borrow_mut() = Some(Rc::downgrade(self)));
    }

    fn drop_window_focus(&self) {
        let Some(win) = self.entry.root().and_downcast::<gtk::Window>() else {
            return;
        };
        win.set_focus(gtk::Widget::NONE);
    }
}

/// Neighbour-search openability: mark gone after trash (no FS preflight on the next filter).
pub fn search_note_removed(
    cell: &Rc<RefCell<Option<Rc<crate::recent_view::RecentContext>>>>,
    path: &std::path::Path,
) {
    if let Some(s) = cell.borrow().as_ref().and_then(|c| c.search.as_ref()) {
        s.note_path_removed(path);
    }
}

/// Neighbour-search openability: reclassify after undo-trash restore.
pub fn search_note_restored(
    cell: &Rc<RefCell<Option<Rc<crate::recent_view::RecentContext>>>>,
    path: &std::path::Path,
) {
    if let Some(s) = cell.borrow().as_ref().and_then(|c| c.search.as_ref()) {
        s.refresh_path_openability(path);
    }
}
