/// Builds the mutually-wired continue-card **Trash** / **Remove** actions; each slot
/// (`cell_t` / `cell_rm`) is filled in as soon as its action exists.
fn build_card_actions(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> (RcPathFn, RcPathFn) {
    let on_trash = build_on_trash(h, do_commit);
    *h.cell_t.borrow_mut() = Some(on_trash.clone());
    let on_remove = build_on_remove(h, do_commit);
    *h.cell_rm.borrow_mut() = Some(on_remove.clone());
    (on_remove, on_trash)
}

/// Continue-card **Trash**: capture an undo snapshot, move the file to Trash, refresh cards.
fn build_on_trash(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> RcPathFn {
    let hh = h.clone();
    let dc = do_commit.clone();
    Rc::new(move |path: &Path| {
        trash_card_action(&hh, &dc, path);
    })
}

/// Continue-card **Trash** click body: snapshot, trash, undo-bar update, card refresh.
fn trash_card_action(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>, path: &Path) {
    if !path.is_file() {
        return;
    }
    let snap = capture_list_remove_undo(path);
    let in_trash = match trash_xdg::trash_local_file_for_undo(path) {
        Err(e) => {
            eprintln!("[rhino] move to trash (continue card): {e}");
            return;
        }
        Ok(loc) => {
            if loc.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            loc
        }
    };
    remove_continue_entry(path);
    if let Some(t) = in_trash {
        h.stack
            .borrow_mut()
            .push(ContinueBarUndo::Trash { snap, in_trash: t });
        sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
        rearm_undo_dismiss(do_commit, &h.timer);
    }
    recent_view::search_note_removed(&h.rbf, path);
    refresh_continue_cards(h);
}

/// Continue-card **Remove**: capture an undo snapshot, drop the entry from the list, refresh cards.
fn build_on_remove(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) -> RcPathFn {
    let hh = h.clone();
    let dc = do_commit.clone();
    Rc::new(move |path: &Path| {
        remove_card_action(&hh, &dc, path);
    })
}

/// Continue-card **Remove** click body: snapshot, list drop, undo-bar update, card refresh, re-arm.
fn remove_card_action(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>, path: &Path) {
    let u = capture_list_remove_undo(path);
    remove_continue_entry(path);
    h.stack.borrow_mut().push(ContinueBarUndo::ListRemove(u));
    sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
    refresh_continue_cards(h);
    rearm_undo_dismiss(do_commit, &h.timer);
}

/// Repaint the continue row through the currently-wired Remove/Trash handlers.
fn refresh_continue_cards(h: &UndoBarHandles) {
    let f = h
        .cell_rm
        .borrow()
        .as_ref()
        .expect("on_remove not wired")
        .clone();
    let t = h
        .cell_t
        .borrow()
        .as_ref()
        .expect("on_trash not wired")
        .clone();
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
