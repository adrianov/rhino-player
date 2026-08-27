type UnitFn = Rc<dyn Fn(()) + 'static>;
type RcPathFn = Rc<dyn Fn(&Path) + 'static>;
type BackfillFn = Rc<dyn Fn(Rc<RecentContext>, Vec<std::path::PathBuf>) + 'static>;
type WarmHoverLeave = Rc<dyn Fn()>;

/// Debounced warm-preload hooks for continue-card pointer enter/leave.
#[derive(Clone)]
pub struct WarmHoverHooks {
    pub enter: RcPathFn,
    pub leave: WarmHoverLeave,
}

/// Per-window state for the recent row: [refill] after background thumbs, [shutdown] on scroll destroy.
pub struct RecentContext {
    chrome_cache: crate::media_probe::ContinueGridCache,
    /// Same box as the grid row; used by [refill].
    row: gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    warm_hover: Option<WarmHoverHooks>,
    /// Neighbour-search state shared with the strip's search box (feature 33).
    pub(crate) search: Option<Rc<SiblingSearchState>>,
    /// Stops workers and poller; cleared in [shutdown].
    pub cancel: Arc<AtomicBool>,
    /// Worker → main: request a [refill] (no GTK types on the [Send] side).
    refill_tx: mpsc::Sender<()>,
    /// Main-loop timer that drains [refill_tx] and calls [refill] on this context.
    poll_id: Rc<RefCell<Option<glib::SourceId>>>,
    /// Background thumb threads (joined in [shutdown]).
    workers: Rc<RefCell<Vec<JoinHandle<()>>>>,
    /// Incremented on each [schedule_thumb_backfill]; stale workers exit between files.
    backfill_gen: Arc<std::sync::atomic::AtomicU64>,
}

impl RecentContext {
    pub(crate) fn warm_hover(&self) -> Option<&WarmHoverHooks> {
        self.warm_hover.as_ref()
    }

    /// Card action wiring for this strip's context (shared by every painter).
    pub(crate) fn strip_actions(&self) -> StripActions {
        StripActions {
            on_open: self.on_open.clone(),
            on_remove: self.on_remove.clone(),
            on_trash: self.on_trash.clone(),
            warm_hover: self.warm_hover.clone(),
        }
    }

    /// Refresh the inline match hint after a query-aware repaint.
    pub(crate) fn note_search_hint(&self) {
        if let Some(s) = &self.search {
            s.note_repaint();
        }
    }

    /// Rebuild the strip with `paths` (already query-resolved or the plain list) and `kind`.
    /// The single card painter: DB-only card data, shared action wiring, chrome cache.
    pub(crate) fn paint(&self, paths: Vec<PathBuf>, kind: StripKind) {
        fill_row(
            &self.row,
            card_data_list(&paths),
            self.strip_actions(),
            Some(&self.chrome_cache),
            kind,
        );
    }

    /// Rebuilds cards: neighbour-substring hits while a query is active, otherwise the five
    /// most-recent watch-later entries.
    pub fn refill(&self) {
        let fallback: Vec<_> = crate::history::load()
            .into_iter()
            .take(CONTINUE_DISPLAY_MAX)
            .collect();
        let plan = strip_plan(self.search.as_deref(), fallback);
        self.paint(plan.paths, plan.kind);
        if plan.searching {
            self.note_search_hint();
        }
    }

    /// Activate one strip path through the shared open handler (Enter on the search box).
    pub(crate) fn open_path(&self, p: &std::path::Path) {
        (self.on_open)(p);
    }

    /// Stops the poller, signals workers to exit, and **detaches** worker joins to a short-lived
    /// background thread (does **not** block the GTK main thread: [media_probe::ensure_thumbnail] can
    /// run many seconds; cancel is checked only between files, not inside libmpv).
    pub fn shutdown(&self) {
        self.cancel.store(false, Ordering::Release);
        crate::glib_source_drop::drop_glib_source(self.poll_id.as_ref());
        let workers: Vec<JoinHandle<()>> = self.workers.borrow_mut().drain(..).collect();
        if workers.is_empty() {
            return;
        }
        if let Err(e) = std::thread::Builder::new()
            .name("rhino-recent-join".to_string())
            .spawn(move || {
                for h in workers {
                    let _ = h.join();
                }
            })
        {
            eprintln!("[rhino] recent: joiner spawn: {e}");
        }
    }
}
