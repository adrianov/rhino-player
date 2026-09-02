/// Builds the mutually-wired continue-card **Trash** / **Remove** actions; each slot
/// (`cell_t` / `cell_rm`) is filled in as soon as its action exists.
fn build_card_actions(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> (RcPathFn, RcPathFn) {
    let on_trash = build_on_trash(h, do_commit);
    *h.cell_t.borrow_mut() = Some(on_trash.clone());
    let on_remove = build_on_remove(h, do_commit);
    *h.cell_rm.borrow_mut() = Some(on_remove.clone());
    (on_remove, on_trash)
}

/// Continue-card **Trash**: feature trash, then undo-bar update and card refresh.
fn build_on_trash(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> RcPathFn {
    let hh = h.clone();
    let dc = do_commit.clone();
    Rc::new(move |path: &Path| {
        trash_card_action(&hh, &dc, path);
    })
}

/// Continue-card **Trash** click body: feature trash, then undo-bar update and card refresh.
fn trash_card_action(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>, path: &Path) {
    let Some((snap, in_trash)) = recent_view::card_trashed(path) else {
        return;
    };
    push_card_trash_undo(h, do_commit, snap, in_trash);
    schedule_refresh_continue_cards(h);
}

fn push_card_trash_undo(
    h: &UndoBarHandles,
    do_commit: &Rc<dyn Fn()>,
    snap: crate::media_probe::ListRemoveUndo,
    in_trash: Option<std::path::PathBuf>,
) {
    let Some(t) = in_trash else {
        return;
    };
    h.stack
        .borrow_mut()
        .push(ContinueBarUndo::Trash { snap, in_trash: t });
    sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
    rearm_undo_dismiss(do_commit, &h.timer);
}

/// Continue-card **Remove**: feature remove, then undo-bar update and card refresh.
fn build_on_remove(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> RcPathFn {
    let hh = h.clone();
    let dc = do_commit.clone();
    Rc::new(move |path: &Path| {
        remove_card_action(&hh, &dc, path);
    })
}

/// Continue-card **Remove** click body: feature remove, then undo-bar update and card refresh.
fn remove_card_action(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>, path: &Path) {
    if let Some(u) = recent_view::card_removed(path) {
        h.stack.borrow_mut().push(ContinueBarUndo::ListRemove(u));
        sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
        rearm_undo_dismiss(do_commit, &h.timer);
    }
    schedule_refresh_continue_cards(h);
}

/// Rebuild the strip after the current GTK handler returns (button click or deferred card press).
fn schedule_refresh_continue_cards(h: &UndoBarHandles) {
    let h = h.clone();
    let _ = glib::idle_add_local_once(move || refresh_continue_cards(&h));
}

/// Repaint the continue row through the currently-wired Remove/Trash handlers.
fn refresh_continue_cards(h: &UndoBarHandles) {
    let Some(f) = h.cell_rm.borrow().as_ref().cloned() else {
        eprintln!("[rhino] continue: refresh skipped (on_remove not wired)");
        return;
    };
    let Some(t) = h.cell_t.borrow().as_ref().cloned() else {
        eprintln!("[rhino] continue: refresh skipped (on_trash not wired)");
        return;
    };
    reflow_continue_cards(
        &h.flow,
        &h.recent,
        h.on_open.clone(),
        f,
        t,
        &h.rbf,
        Rc::clone(&h.cache),
    );
}
