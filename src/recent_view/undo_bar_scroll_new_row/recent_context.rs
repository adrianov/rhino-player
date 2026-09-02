type UnitFn = Rc<dyn Fn(()) + 'static>;
type RcPathFn = Rc<dyn Fn(&Path) + 'static>;
type BackfillFn = Rc<dyn Fn(Rc<RecentContext>, Vec<PathBuf>) + 'static>;
type WarmHoverLeave = Rc<dyn Fn()>;

/// Debounced warm-preload hooks for continue-card pointer enter/leave.
#[derive(Clone)]
pub struct WarmHoverHooks {
    pub enter: RcPathFn,
    pub leave: WarmHoverLeave,
}

/// Per-window state for the recent row: strip paint, neighbour search, thumb backfill.
pub struct RecentContext {
    chrome_cache: crate::media_probe::ContinueGridCache,
    row: gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    warm_hover: Option<WarmHoverHooks>,
    /// Neighbour-search state shared with the strip's search box (feature 33).
    pub(crate) search: Option<Rc<SiblingSearchState>>,
    /// Strip overlays for width sync ([fill_row]); index 0 is Open Video.
    cards: Rc<RefCell<Vec<gtk::Overlay>>>,
    /// Media paths for `cards[1..]` — used by in-place thumb apply ([live_card]).
    media_paths: RefCell<Vec<PathBuf>>,
    /// Width-notify for [cards] is connected once.
    size_wired: std::cell::Cell<bool>,
    /// Thumbnail workers + event-driven ready-path delivery ([ThumbBackfill] in live_card).
    pub(crate) thumbs: ThumbBackfill,
}

impl RecentContext {
    pub(super) fn from_hooks(row: &gtk::Box, hooks: ContinueStripHooks) -> Self {
        Self {
            chrome_cache: hooks.chrome_cache,
            row: row.clone(),
            on_open: hooks.on_open,
            on_remove: hooks.on_remove,
            on_trash: hooks.on_trash,
            warm_hover: hooks.warm_hover,
            search: hooks.search,
            cards: Rc::new(RefCell::new(Vec::new())),
            media_paths: RefCell::new(Vec::new()),
            size_wired: std::cell::Cell::new(false),
            thumbs: ThumbBackfill::new(),
        }
    }

    /// Bind neighbour-search + thumb flush after the context `Rc` exists.
    pub(super) fn finish_spawn(self: &Rc<Self>) {
        if let Some(s) = &self.search {
            s.bind_ctx(Rc::downgrade(self));
        }
        ThumbBackfill::install_flush(self);
    }

    pub(crate) fn warm_hover(&self) -> Option<&WarmHoverHooks> {
        self.warm_hover.as_ref()
    }

    pub(crate) fn strip_actions(&self) -> StripActions {
        StripActions {
            on_open: self.on_open.clone(),
            on_remove: self.on_remove.clone(),
            on_trash: self.on_trash.clone(),
            warm_hover: self.warm_hover.clone(),
        }
    }

    pub(crate) fn note_search_hint(&self) {
        if let Some(s) = &self.search {
            s.note_repaint();
        }
    }

    /// Rebuild the strip. No-op while a search draft is settling; neighbour paints with the
    /// same paths and stored progress are skipped inside [SiblingSearchState].
    pub(crate) fn paint(&self, paths: Vec<PathBuf>, kind: StripKind) {
        if self.search.as_ref().is_some_and(|s| s.typing_pending()) {
            return;
        }
        if kind == StripKind::NeighbourHits {
            let Some(s) = &self.search else {
                return;
            };
            if !s.begin_hits_paint(&paths) {
                return;
            }
        } else if let Some(s) = &self.search {
            s.clear_hits_paint();
        }
        fill_row(
            &self.row,
            card_data_list(&paths),
            self.strip_actions(),
            Some(&self.chrome_cache),
            kind,
            &self.cards,
            &self.size_wired,
        );
        *self.media_paths.borrow_mut() = paths;
    }

    /// Paint the query-aware strip and arm thumb workers for those paths.
    pub(crate) fn apply_strip(self: &Rc<Self>) {
        let paths = self.paint_strip();
        self.schedule_thumbs(paths);
    }

    /// Visible strip first, then a reserved next lucky handful (feature 33).
    pub(crate) fn schedule_thumbs(&self, mut paths: Vec<PathBuf>) {
        if let Some(s) = &self.search {
            s.append_lucky_warm(&mut paths);
        }
        self.thumbs.schedule(paths);
    }

    fn paint_strip(&self) -> Vec<PathBuf> {
        let fallback: Vec<_> = crate::history::load()
            .into_iter()
            .take(CONTINUE_DISPLAY_MAX)
            .collect();
        let plan = strip_plan(self.search.as_deref(), fallback);
        self.paint(plan.paths.clone(), plan.kind);
        if plan.searching {
            self.note_search_hint();
        }
        plan.paths
    }

    pub(crate) fn open_path(&self, p: &std::path::Path) {
        (self.on_open)(p);
    }

    pub fn shutdown(&self) {
        self.thumbs.shutdown();
    }
}
