/// Card actions and shared chrome cache for continue-strip cards.
pub struct ContinueStripHooks {
    pub on_open: RcPathFn,
    pub on_remove: RcPathFn,
    pub on_trash: RcPathFn,
    pub warm_hover: Option<WarmHoverHooks>,
    pub chrome_cache: crate::media_probe::ContinueGridCache,
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

/// Fresh [RecentContext]: channels, worker registry, and the 32ms refill poll source.
fn spawn_recent_context(row: &gtk::Box, hooks: ContinueStripHooks) -> Rc<RecentContext> {
    let (cancel, backfill_gen) = fresh_backfill_counters();
    let (refill_tx, refill_rx) = mpsc::channel();
    let ctx = Rc::new(RecentContext {
        chrome_cache: hooks.chrome_cache,
        row: row.clone(),
        on_open: hooks.on_open,
        on_remove: hooks.on_remove,
        on_trash: hooks.on_trash,
        warm_hover: hooks.warm_hover,
        cancel,
        refill_tx,
        poll_id: Rc::new(RefCell::new(None)),
        workers: Rc::new(RefCell::new(Vec::new())),
        backfill_gen,
    });
    let id = spawn_refill_poll(&ctx, refill_rx);
    *ctx.poll_id.borrow_mut() = Some(id);
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

/// Hand on hover, primary click triggers [act]. [show_on_hover] (Remove / Move to Trash) shows on hover.
/// Uses [PropagationPhase::Target] so nested [gtk::Button]s receive the click first.
fn add_click_and_pointer(
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    act: UnitFn,
    show_on_hover: &[gtk::Button],
    warm_hover: Option<&WarmHoverHooks>,
) {
    attach_click_gesture(card, act);
    attach_hover_pointer(card, path, show_on_hover, warm_hover);
}

fn attach_click_gesture(card: &impl IsA<gtk::Widget>, act: UnitFn) {
    card.as_ref().set_can_target(true);
    let g = gtk::GestureClick::new();
    g.set_button(1);
    g.set_propagation_phase(gtk::PropagationPhase::Target);
    let act = act.clone();
    g.connect_pressed(move |_, n, _x, _y| {
        if n == 1 {
            act(());
        }
    });
    card.as_ref().add_controller(g);
}

/// Pointer cursor while hovering; reveals [show_on_hover] buttons and fires warm hooks.
fn attach_hover_pointer(
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    show_on_hover: &[gtk::Button],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let m = gtk::EventControllerMotion::new();
    wire_hover_enter(&m, card, path, show_on_hover, warm_hover);
    wire_hover_leave(&m, card, show_on_hover, warm_hover);
    card.as_ref().add_controller(m);
}

fn wire_hover_enter(
    m: &gtk::EventControllerMotion,
    card: &impl IsA<gtk::Widget>,
    path: &Path,
    show_on_hover: &[gtk::Button],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let c = card.as_ref().clone();
    let show: Vec<gtk::Button> = show_on_hover.to_vec();
    let warm_enter = warm_hover.map(|h| h.enter.clone());
    let warm_path = path.to_path_buf();
    m.connect_enter(move |_, _x, _y| hover_enter(&c, &show, warm_enter.as_ref(), &warm_path));
}

fn wire_hover_leave(
    m: &gtk::EventControllerMotion,
    card: &impl IsA<gtk::Widget>,
    show_on_hover: &[gtk::Button],
    warm_hover: Option<&WarmHoverHooks>,
) {
    let c = card.as_ref().clone();
    let hide: Vec<gtk::Button> = show_on_hover.to_vec();
    let warm_leave = warm_hover.map(|h| h.leave.clone());
    m.connect_leave(move |_| hover_leave(&c, &hide, warm_leave.as_ref()));
}

/// Enter the card: pointer cursor, reveal hover actions, fire the warm-preload hook.
fn hover_enter(c: &gtk::Widget, show: &[gtk::Button], warm_enter: Option<&RcPathFn>, path: &Path) {
    c.set_cursor_from_name(Some("pointer"));
    for b in show {
        b.set_visible(true);
    }
    if let Some(f) = warm_enter {
        f(path);
    }
}

/// Leave the card: reset cursor, hide hover actions, end warm preload.
fn hover_leave(c: &gtk::Widget, hide: &[gtk::Button], warm_leave: Option<&WarmHoverLeave>) {
    c.set_cursor_from_name(None);
    for b in hide {
        b.set_visible(false);
    }
    if let Some(f) = warm_leave {
        f();
    }
}

include!("backfill_context_schedule/thumb_worker.rs");
