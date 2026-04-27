/// Creates or reuses a [RecentContext] in [cell] (one per window).
pub fn ensure_recent_backfill(
    cell: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
) -> Rc<RecentContext> {
    if let Some(c) = cell.borrow().as_ref() {
        return Rc::clone(c);
    }
    let cancel = Arc::new(AtomicBool::new(true));
    let (refill_tx, refill_rx) = mpsc::channel();
    let ctx = Rc::new(RecentContext {
        row: row.clone(),
        on_open,
        on_remove,
        on_trash,
        cancel: cancel.clone(),
        refill_tx,
        poll_id: Rc::new(RefCell::new(None)),
        workers: Rc::new(RefCell::new(Vec::new())),
    });
    let c_poll = Rc::clone(&ctx);
    // [Receiver] is main-thread only; the timer callback runs on the GTK main thread.
    let rxm = Rc::new(RefCell::new(refill_rx));
    let c_rx = Rc::clone(&rxm);
    let id = glib::source::timeout_add_local(Duration::from_millis(32), move || {
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
    });
    *ctx.poll_id.borrow_mut() = Some(id);
    *cell.borrow_mut() = Some(Rc::clone(&ctx));
    ctx
}

/// For each path, if the file is present and the DB has no up-to-date thumb, runs [media_probe::ensure_thumbnail] on a **worker** thread, then [RecentContext::refill] on the main loop via a [Send] channel.
/// Safe to call from the main thread: does not block on libmpv.
pub fn schedule_thumb_backfill(ctx: Rc<RecentContext>, paths: Vec<std::path::PathBuf>) {
    let tx = ctx.refill_tx.clone();
    let c = ctx.cancel.clone();
    let h = std::thread::spawn(move || {
        for p in paths {
            if !c.load(Ordering::Acquire) {
                return;
            }
            if !p.exists() {
                continue;
            }
            let can = match std::fs::canonicalize(&p) {
                Ok(c) => c,
                _ => continue,
            };
            if media_probe::cached_thumbnail_for_path(&can).is_some() {
                continue;
            }
            let _ = media_probe::ensure_thumbnail(&can);
            if !c.load(Ordering::Acquire) {
                return;
            }
            if tx.send(()).is_err() {
                return;
            }
        }
    });
    ctx.workers.borrow_mut().push(h);
}

/// Hand on hover, primary click triggers [act]. [show_on_hover] (e.g. trash + remove) is shown on hover.
/// Uses [PropagationPhase::Target] so nested [gtk::Button]s receive the click first.
fn add_click_and_pointer(
    card: &impl IsA<gtk::Widget>,
    debug_path: &str,
    act: UnitFn,
    show_on_hover: &[gtk::Button],
) {
    card.as_ref().set_can_target(true);
    let g = gtk::GestureClick::new();
    g.set_button(1);
    g.set_propagation_phase(gtk::PropagationPhase::Target);
    let act = act.clone();
    let p = debug_path.to_string();
    g.connect_pressed(move |_, n, _x, _y| {
        eprintln!("[rhino] recent: gesture pressed n={n} path={p}");
        if n == 1 {
            eprintln!("[rhino] recent: invoking open/remove handler");
            act(());
        } else {
            eprintln!("[rhino] recent: ignored n!=1 (if stuck, n may be wrong for this GTK/WM)");
        }
    });
    card.as_ref().add_controller(g);

    let c = card.as_ref().clone();
    let show: Vec<gtk::Button> = show_on_hover.to_vec();
    let m = gtk::EventControllerMotion::new();
    m.connect_enter(move |_, _x, _y| {
        c.set_cursor_from_name(Some("pointer"));
        for b in &show {
            b.set_visible(true);
        }
    });
    let c = card.as_ref().clone();
    let hide: Vec<gtk::Button> = show_on_hover.to_vec();
    m.connect_leave(move |_| {
        c.set_cursor_from_name(None);
        for b in &hide {
            b.set_visible(false);
        }
    });
    card.as_ref().add_controller(m);
}
