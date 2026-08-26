include!("undo_commit.rs");
include!("undo_card_actions.rs");
include!("undo_button.rs");

fn wire_recent_undo(ctx: RecentUndoCtx) -> RecentUndoWiring {
    let RecentUndoCtx {
        player,
        recent: recent_scrl,
        flow: flow_recent,
        undo_shell,
        undo_label,
        undo_btn,
        undo_close,
        on_open,
        want_recent,
        warm_hover,
        continue_grid_cache,
    } = ctx;

    let recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>> = Rc::new(RefCell::new(None));
    let (pending_recent_backfill, recent_backfill_start) =
        wire_recent_backfill(&recent_backfill, &player, &recent_scrl);

    let mut h = wire_undo_handles(
        undo_shell,
        undo_label,
        undo_btn,
        flow_recent,
        recent_scrl.clone(),
        on_open,
        continue_grid_cache,
    );
    h.rbf = recent_backfill.clone();
    let do_commit = build_do_commit(&h);
    let (on_remove, on_trash) = build_card_actions(&h, &do_commit);
    wire_undo_button(&h, &do_commit);
    arm_undo_close_button(&undo_close, &do_commit);

    if want_recent {
        fill_initial_continue_strip(
            &h,
            &on_remove,
            &on_trash,
            &warm_hover,
            recent_backfill_start,
        );
    }

    RecentUndoWiring {
        recent_backfill,
        pending_recent_backfill,
        undo_remove_stack: h.stack.clone(),
        undo_timer: h.timer.clone(),
        do_commit,
        on_remove,
        on_trash,
    }
}

type RecentBackfillChannel = (
    Rc<RefCell<Option<RecentBackfillJob>>>,
    Rc<dyn Fn(Rc<RecentContext>, Vec<PathBuf>)>,
);

/// Recent-backfill channel: deferred job slot + start hook; the grid destroy drops the pending
/// job and shuts the backfill context down.
fn wire_recent_backfill(
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_scrl: &gtk::Box,
) -> RecentBackfillChannel {
    let pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>> =
        Rc::new(RefCell::new(None));
    {
        let rb = rbf.clone();
        let pending = pending_recent_backfill.clone();
        recent_scrl.connect_destroy(move |_| {
            shutdown_recent_backfill(&rb, &pending);
        });
    }
    let p = player.clone();
    let pending = pending_recent_backfill.clone();
    let start = Rc::new(move |ctx: Rc<RecentContext>, paths: Vec<PathBuf>| {
        schedule_or_defer_recent_backfill(&p, &pending, ctx, paths)
    });
    (pending_recent_backfill, start)
}

/// Drop a queued backfill job and stop the running backfill context.
fn shutdown_recent_backfill(
    rb: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    pending: &Rc<RefCell<Option<RecentBackfillJob>>>,
) {
    pending.borrow_mut().take();
    if let Some(ctx) = rb.borrow_mut().take() {
        ctx.shutdown();
    }
}

/// Dismiss (**×**) button commits the topmost step immediately.
fn arm_undo_close_button(undo_close: &gtk::Button, do_commit: &Rc<dyn Fn()>) {
    let dc = do_commit.clone();
    undo_close.connect_clicked(move |_| {
        dc();
    });
}

/// Initial continue-strip paint when booting straight into the grid.
fn fill_initial_continue_strip(
    h: &UndoBarHandles,
    on_remove: &RcPathFn,
    on_trash: &RcPathFn,
    warm_hover: &Option<recent_view::WarmHoverHooks>,
    recent_backfill_start: Rc<dyn Fn(Rc<RecentContext>, Vec<PathBuf>)>,
) {
    let paths5: Vec<PathBuf> = history::load()
        .into_iter()
        .take(crate::recent_view::CONTINUE_DISPLAY_MAX)
        .collect();
    recent_view::fill_continue_strip(
        &h.flow,
        paths5,
        recent_view::ContinueStripHooks {
            on_open: h.on_open.clone(),
            on_remove: on_remove.clone(),
            on_trash: on_trash.clone(),
            warm_hover: warm_hover.clone(),
            chrome_cache: Rc::clone(&h.cache),
        },
        h.rbf.clone(),
        recent_backfill_start,
    );
}
