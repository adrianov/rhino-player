/// Card actions and shared chrome cache for continue-strip cards.
pub struct ContinueStripHooks {
    pub on_open: RcPathFn,
    pub on_remove: RcPathFn,
    pub on_trash: RcPathFn,
    pub warm_hover: Option<WarmHoverHooks>,
    pub chrome_cache: crate::media_probe::ContinueGridCache,
    /// Neighbour-search state (feature 33); bound to the context at first spawn.
    pub search: Option<Rc<SiblingSearchState>>,
}

/// Fresh [RecentContext]: channels, worker registry, and the 32 ms refill poll source.
/// Binds [SiblingSearchState] to the context so search input can repaint the strip.
fn spawn_recent_context(row: &gtk::Box, hooks: ContinueStripHooks) -> Rc<RecentContext> {
    let (refill_tx, refill_rx) = mpsc::channel();
    let ctx = Rc::new(recent_from_hooks(row, hooks, refill_tx));
    wire_search_poll(&ctx, refill_rx);
    ctx
}

/// Build the strip context; search bind and refill poll are wired by [wire_search_poll].
fn recent_from_hooks(
    row: &gtk::Box,
    hooks: ContinueStripHooks,
    refill_tx: mpsc::Sender<()>,
) -> RecentContext {
    let (cancel, backfill_gen) = fresh_backfill_counters();
    RecentContext {
        chrome_cache: hooks.chrome_cache,
        row: row.clone(),
        on_open: hooks.on_open,
        on_remove: hooks.on_remove,
        on_trash: hooks.on_trash,
        warm_hover: hooks.warm_hover,
        search: hooks.search,
        cancel,
        refill_tx,
        poll_id: Rc::new(RefCell::new(None)),
        workers: Rc::new(RefCell::new(Vec::new())),
        backfill_gen,
    }
}

fn wire_search_poll(ctx: &Rc<RecentContext>, refill_rx: mpsc::Receiver<()>) {
    if let Some(s) = &ctx.search {
        s.bind_ctx(Rc::downgrade(ctx));
    }
    *ctx.poll_id.borrow_mut() = Some(spawn_refill_poll(ctx, refill_rx));
}

/// Creates or reuses a [RecentContext] in [cell] (one per window).
pub fn ensure_recent_backfill(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    hooks: ContinueStripHooks,
) -> Rc<RecentContext> {
    if let Some(c) = cell.borrow().as_ref() {
        return Rc::clone(c);
    }
    let ctx = spawn_recent_context(row, hooks);
    *cell.borrow_mut() = Some(Rc::clone(&ctx));
    ctx
}

/// Fresh cancellation flag and backfill generation counter for a new context.
fn fresh_backfill_counters() -> (Arc<AtomicBool>, Arc<std::sync::atomic::AtomicU64>) {
    (
        Arc::new(AtomicBool::new(true)),
        Arc::new(std::sync::atomic::AtomicU64::new(0)),
    )
}

fn spawn_refill_poll(ctx: &Rc<RecentContext>, refill_rx: mpsc::Receiver<()>) -> glib::SourceId {
    let c_poll = Rc::clone(ctx);
    // [Receiver] is main-thread only; the timer callback runs on the GTK main thread.
    let rxm = Rc::new(RefCell::new(refill_rx));
    let c_rx = Rc::clone(&rxm);
    glib::source::timeout_add_local(Duration::from_millis(32), move || {
        let mut n = 0u32;
        {
            let g = c_rx.borrow_mut();
            while g.try_recv().is_ok() {
                n += 1;
            }
        }
        if n > 0 {
            c_poll.refill();
        }
        glib::ControlFlow::Continue
    })
}

/// For each displayed continue path (at most [CONTINUE_DISPLAY_MAX]), if the file is present and
/// the DB has no up-to-date WebP thumb, runs [media_probe::ensure_thumbnail] on a **worker**
/// thread, then [RecentContext::refill] on the main loop via a [Send] channel.
/// Safe to call from the main thread: does not block on libmpv.
pub fn schedule_thumb_backfill(ctx: Rc<RecentContext>, paths: Vec<std::path::PathBuf>) {
    let paths: Vec<_> = paths.into_iter().take(CONTINUE_DISPLAY_MAX).collect();
    if paths.is_empty() {
        return;
    }
    let gen = ctx
        .backfill_gen
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let tx = ctx.refill_tx.clone();
    let c = ctx.cancel.clone();
    let gen_watch = ctx.backfill_gen.clone();
    let h = std::thread::spawn(move || run_thumb_worker(paths, gen, c, tx, gen_watch));
    ctx.workers.borrow_mut().push(h);
}

/// True when a newer backfill generation started or shutdown cancelled the workers.
fn thumb_gen_cancelled(gen_watch: &std::sync::atomic::AtomicU64, gen: u64, c: &AtomicBool) -> bool {
    gen_watch.load(std::sync::atomic::Ordering::Acquire) != gen || !c.load(Ordering::Acquire)
}

include!("backfill_context_schedule/card_pointer.rs");
include!("backfill_context_schedule/thumb_worker.rs");
