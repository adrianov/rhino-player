/// **Undo** button: pop one step and restore it; on failure push the entry back and keep the bar.
fn wire_undo_button(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) {
    let hh = h.clone();
    let dc = do_commit.clone();
    h.btn.connect_clicked(move |_| {
        undo_button_clicked(&hh, &dc);
    });
}

/// One **Undo** click body: pop, restore, refresh bar + cards, re-arm dismissal while steps remain.
fn undo_button_clicked(h: &UndoBarHandles, do_commit: &Rc<dyn Fn()>) {
    cancel_undo_timer(h.timer.as_ref());
    let Some(undo) = h.stack.borrow_mut().pop() else {
        return;
    };
    if let Err(e) = apply_bar_undo(&undo) {
        eprintln!("[rhino] undo: {e}");
        h.stack.borrow_mut().push(undo);
        return;
    }
    restore_undo_step(h, undo);
    if !h.stack.borrow().is_empty() {
        rearm_undo_dismiss(do_commit, &h.timer);
    }
}

/// Persist the restored path and repaint bar + continue row.
fn restore_undo_step(h: &UndoBarHandles, undo: ContinueBarUndo) {
    let path = undo.target_path().to_path_buf();
    let was_trash = matches!(&undo, ContinueBarUndo::Trash { .. });
    history::record(&path);
    if was_trash {
        recent_view::search_note_restored(&h.rbf, &path);
    }
    sync_undo_bar(&h.label, &h.btn, &h.shell, &h.stack);
    h.recent.set_visible(true);
    refresh_continue_cards(h);
}
