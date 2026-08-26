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

    /// Rebuilds cards from the current history (first five paths).
    pub fn refill(&self) {
        let paths: Vec<std::path::PathBuf> = crate::history::load()
            .into_iter()
            .take(CONTINUE_DISPLAY_MAX)
            .collect();
        let v: Vec<CardData> = card_data_list(&paths);
        fill_row(
            &self.row,
            v,
            self.on_open.clone(),
            self.on_remove.clone(),
            self.on_trash.clone(),
            self.warm_hover.as_ref(),
            Some(&self.chrome_cache),
        );
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
