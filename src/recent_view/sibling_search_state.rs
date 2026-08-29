// [SiblingSearchState] — neighbour-search query, index, and strip paint bookkeeping.
// Reactive typing (debounce / commit) lives in [sibling_search_input.rs].
// Loaded as `#[path]` from sibling_search so module-ABC stays off the scan hub.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::glib::prelude::CastNone;
use gtk::prelude::{GtkWindowExt, WidgetExt};

use super::{
    build_neighbour_index, classify_openable, index_fill_once, present_name_hits, take_capped,
    NeighbourEntry,
};

/// Settled filter delay after typing stops (feature 33).
pub(super) const TYPE_DEBOUNCE_MS: u64 = 200;

type CtxSlot = RefCell<Option<Weak<crate::recent_view::RecentContext>>>;

/// Query text, sibling-file index, and result bookkeeping for one window.
pub(crate) struct SiblingSearchState {
    pub(super) entry: gtk::SearchEntry,
    hint: gtk::Label,
    /// Committed filter (drives strip paint). Entry text is draft until debounce.
    pub(super) query: RefCell<String>,
    /// Session neighbour index (path + openability); filled once per window.
    index: RefCell<Vec<NeighbourEntry>>,
    scanned: Cell<bool>,
    last_hits: RefCell<Option<(usize, bool)>>,
    /// Last neighbour paths painted; identical commits skip [fill_row].
    painted: RefCell<Option<Vec<PathBuf>>>,
    pub(super) ctx: CtxSlot,
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
}

#[path = "sibling_search_input.rs"]
mod input;

impl SiblingSearchState {
    pub(super) fn new(entry: gtk::SearchEntry, hint: gtk::Label) -> Rc<Self> {
        Rc::new(Self {
            entry,
            hint,
            query: RefCell::new(String::new()),
            index: RefCell::default(),
            scanned: Cell::new(false),
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
        let files = self.neighbour_index();
        let (hits, capped) = take_capped(present_name_hits(&files, &q));
        *self.last_hits.borrow_mut() = Some((hits.len(), capped));
        Some(hits)
    }

    pub(super) fn neighbour_index(&self) -> Vec<NeighbourEntry> {
        index_fill_once(&self.scanned, &self.index, build_neighbour_index)
    }

    /// Mark a path unopenable after trash / removal so the next filter skips it without FS I/O.
    pub(crate) fn note_path_removed(&self, path: &std::path::Path) {
        if let Some(e) = self
            .index
            .borrow_mut()
            .iter_mut()
            .find(|e| e.path == path)
        {
            e.openable = false;
        }
        self.clear_hits_paint();
    }

    /// Re-run open preflight for one indexed path (e.g. undo trash restore).
    pub(crate) fn refresh_path_openability(&self, path: &std::path::Path) {
        if let Some(e) = self
            .index
            .borrow_mut()
            .iter_mut()
            .find(|e| e.path == path)
        {
            e.openable = classify_openable(&e.path);
        }
        self.clear_hits_paint();
    }

    pub(crate) fn note_repaint(&self) {
        self.hint.set_text(&match (*self.last_hits.borrow()).filter(|_| self.searching()) {
            None => String::new(),
            Some((n, true)) => format!("{n}+ matches"),
            Some((0, false)) => "No matches".to_string(),
            Some((n, false)) => format!("{n} match{}", if n == 1 { "" } else { "es" }),
        });
    }

    /// Browse strip shown ↔ search may take focus; when hidden for playback, drop IM/caret focus
    /// so gdk-macos cannot paint typed glyphs over the video.
    pub(crate) fn sync_browse_visible(&self, visible: bool) {
        if visible {
            self.entry.set_can_focus(true);
            return;
        }
        self.drop_window_focus();
        self.entry.set_can_focus(false);
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
