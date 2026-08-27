// Undo-snackbar presentation for the continue bar: label line, tooltip, auto-dismiss timer.
// Split out of `eof_advance_and_browse_ctx.rs` (paint/backfill owns the rest).

fn cancel_undo_timer(src: &RefCell<Option<glib::source::SourceId>>) {
    drop_glib_source(src);
}
/// LIFO stack: label shows the file that **Undo** will restore; dismiss / timeout discards that undo target only.
fn sync_undo_bar(
    label: &gtk::Label,
    btn: &gtk::Button,
    shell: &gtk::Box,
    stack: &RefCell<Vec<ContinueBarUndo>>,
) {
    let n = stack.borrow().len();
    shell.set_visible(n > 0);
    if n == 0 {
        label.set_label("");
        btn.set_tooltip_text(None);
        return;
    }
    set_undo_tooltip(btn, n);
    set_undo_top_label(label, stack);
}

/// Undo button tooltip: single-step hint, or a step counter while several entries remain.
fn set_undo_tooltip(btn: &gtk::Button, n: usize) {
    match n {
        1 => btn.set_tooltip_text(Some(
            "Restore to the list (and from Trash if that was the last action)",
        )),
        n => {
            let s = format!("Undo newest first — {n} steps left");
            btn.set_tooltip_text(Some(s.as_str()));
        }
    }
}

/// Label line naming the file the next **Undo** will restore and how it left the list.
fn set_undo_top_label(label: &gtk::Label, stack: &RefCell<Vec<ContinueBarUndo>>) {
    if let Some(p) = stack.borrow().last() {
        let (name, tail) = match p {
            ContinueBarUndo::ListRemove(u) => (
                u.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "removed from list",
            ),
            ContinueBarUndo::Trash { snap, .. } => (
                snap.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "moved to Trash",
            ),
        };
        let line = format!("\u{201c}{name}\u{201d} {tail}");
        label.set_label(&line);
    }
}

fn rearm_undo_dismiss(
    do_commit: &Rc<dyn Fn() + 'static>,
    undo_source: &Rc<RefCell<Option<glib::source::SourceId>>>,
) {
    cancel_undo_timer(undo_source.as_ref());
    let c = do_commit.clone();
    let slot = Rc::clone(undo_source);
    *undo_source.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
        crate::glib_source_drop::finish_glib_source(slot.as_ref());
        c();
        glib::ControlFlow::Break
    }));
}
