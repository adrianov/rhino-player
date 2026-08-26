/// Weak self-slot letting [build_do_commit]'s callback re-arm its own dismiss timer.
type DismissTopRef = Rc<RefCell<Option<Weak<dyn Fn() + 'static>>>>;

/// Late-bound card action slot: **Remove**/**Trash** closures reference each other, so the
/// `RcPathFn` is filled in after both are built.
type UndoSlot = Rc<RefCell<Option<RcPathFn>>>;

/// Shared widget/state handles threaded through every undo-bar closure. Cloned per closure;
/// the card action slots are filled in once both actions exist.
#[derive(Clone)]
struct UndoBarHandles {
    shell: gtk::Box,
    label: gtk::Label,
    btn: gtk::Button,
    /// Auto-dismiss timer for the topmost undo step.
    timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    /// LIFO stack of removed/trashed entries, newest at the end.
    stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    flow: gtk::Box,
    recent: gtk::Box,
    on_open: RcPathFn,
    rbf: Rc<RefCell<Option<Rc<RecentContext>>>>,
    cache: crate::media_probe::ContinueGridCache,
    cell_rm: UndoSlot,
    cell_t: UndoSlot,
}

/// Bundle the undo-bar widget/state handles (empty backfill channel + late-bound slots).
fn wire_undo_handles(
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    flow: gtk::Box,
    recent: gtk::Box,
    on_open: RcPathFn,
    cache: crate::media_probe::ContinueGridCache,
) -> UndoBarHandles {
    UndoBarHandles {
        shell: undo_shell,
        label: undo_label,
        btn: undo_btn,
        timer: Rc::new(RefCell::new(None)),
        stack: Rc::new(RefCell::new(Vec::<ContinueBarUndo>::new())),
        flow,
        recent,
        on_open,
        rbf: Rc::new(RefCell::new(None)),
        cache,
        cell_rm: Rc::new(RefCell::new(None)),
        cell_t: Rc::new(RefCell::new(None)),
    }
}

/// Builds the auto-dismiss commit: pops the newest undo entry and refreshes the bar.
fn build_do_commit(h: &UndoBarHandles) -> Rc<dyn Fn() + 'static> {
    let weak: DismissTopRef = Rc::new(RefCell::new(None));
    let hh = h.clone();
    let wk = weak.clone();
    let do_commit: Rc<dyn Fn() + 'static> = Rc::new(move || {
        dismiss_top_undo_step(&hh, &wk);
    });
    *weak.borrow_mut() = Some(Rc::downgrade(&do_commit));
    do_commit
}

/// Dismissal (timeout / close): drop the newest undo step; refresh the bar and re-arm the
/// 10 s timer while steps remain, through the weak self-slot so dismissal stops the chain.
fn dismiss_top_undo_step(h: &UndoBarHandles, weak: &DismissTopRef) {
    cancel_undo_timer(h.timer.as_ref());
    if h.stack.borrow_mut().pop().is_none() {
        return;
    }
    sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
    if !h.stack.borrow().is_empty() {
        rearm_dismiss_from_weak(h, weak);
    }
}

/// Re-arm the 10 s auto-dismiss through the weak self-slot; a chained dismissal stops here.
fn rearm_dismiss_from_weak(h: &UndoBarHandles, weak: &DismissTopRef) {
    let Some(f) = weak.borrow().as_ref().and_then(|w| w.upgrade()) else {
        return;
    };
    let ht = h.clone();
    *h.timer.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
        crate::glib_source_drop::finish_glib_source(ht.timer.as_ref());
        f();
        glib::ControlFlow::Break
    }));
}
