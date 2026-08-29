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
    let (cancel, backfill_gen) = fresh_backfill_counters();
    let ctx = Rc::new(RecentContext::from_hooks(
        row,
        hooks,
        refill_tx,
        cancel,
        backfill_gen,
    ));
    wire_search_poll(&ctx, refill_rx);
    ctx
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

/// Ensure the strip context, then paint the query-aware strip and arm thumb workers.
/// Single entry for remove/undo/browse-back so search hits get the same backfill as continue.
pub fn ensure_apply_strip(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    hooks: ContinueStripHooks,
) {
    ensure_recent_backfill(cell, row, hooks).apply_strip();
}

/// Hooks for a strip paint: warm-hover and neighbour-search ride along with the last context.
pub fn strip_hooks_from_cell(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    chrome_cache: crate::media_probe::ContinueGridCache,
) -> ContinueStripHooks {
    let ctx = cell.borrow();
    ContinueStripHooks {
        on_open,
        on_remove,
        on_trash,
        warm_hover: ctx.as_ref().and_then(|c| c.warm_hover().cloned()),
        chrome_cache,
        search: ctx.as_ref().and_then(|c| c.search.as_ref().map(Rc::clone)),
    }
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

/// For each path still missing a fresh WebP thumb, run [media_probe::ensure_thumbnail] on a
/// **worker** thread, then [RecentContext::refill] on the main loop via a [Send] channel.
/// Callers pass the painted strip only (already capped). Safe from the main thread.
pub fn schedule_thumb_backfill(ctx: Rc<RecentContext>, paths: Vec<std::path::PathBuf>) {
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
