fn nudge_mpv_volume(mpv: &Mpv, delta: f64) {
    let max = mpv
        .get_property::<f64>("volume-max")
        .unwrap_or(100.0)
        .max(1.0);
    let cur = mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let nv = (cur + delta).clamp(0.0, max);
    let _ = mpv.set_property("volume", nv);
    if nv > 0.5 {
        let _ = mpv.set_property("mute", false);
    }
}

/// Rebuild the continue row from [history] after a remove or undo.
fn reflow_continue_cards(
    row: &gtk::Box,
    recent: &gtk::Box,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    chrome_cache: crate::media_probe::ContinueGridCache,
) {
    let r: Vec<PathBuf> = history::load()
        .into_iter()
        .take(crate::recent_view::CONTINUE_DISPLAY_MAX)
        .collect();
    recent.set_visible(true);
    repaint_continue_row(row, rbf, &r, &on_open, &on_remove, &on_trash, &chrome_cache);
}

/// Repaint a continue row from card data and wire its thumbnail backfill (idle body of
/// [schedule_continue_grid_refill], direct body of [reflow_continue_cards]).
fn repaint_continue_row(
    row: &gtk::Box,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    paths: &[PathBuf],
    on_open: &RcPathFn,
    on_remove: &RcPathFn,
    on_trash: &RcPathFn,
    chrome_cache: &crate::media_probe::ContinueGridCache,
) {
    let v: Vec<CardData> = card_data_list(paths);
    let warm = rbf.borrow().as_ref().and_then(|c| c.warm_hover().cloned());
    recent_view::fill_row(
        row,
        v,
        on_open.clone(),
        on_remove.clone(),
        on_trash.clone(),
        warm.as_ref(),
        Some(chrome_cache),
    );
    backfill_continue_row(rbf, row, paths, on_open, on_remove, on_trash, chrome_cache);
}

/// Wire thumbnail backfill for a freshly painted continue row.
fn backfill_continue_row(
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    row: &gtk::Box,
    paths: &[PathBuf],
    on_open: &RcPathFn,
    on_remove: &RcPathFn,
    on_trash: &RcPathFn,
    chrome_cache: &crate::media_probe::ContinueGridCache,
) {
    let warm_ctx = rbf.borrow().as_ref().and_then(|c| c.warm_hover().cloned());
    let n = recent_view::ensure_recent_backfill(
        rbf,
        row,
        recent_view::ContinueStripHooks {
            on_open: on_open.clone(),
            on_remove: on_remove.clone(),
            on_trash: on_trash.clone(),
            warm_hover: warm_ctx,
            chrome_cache: Rc::clone(chrome_cache),
        },
    );
    recent_view::schedule_thumb_backfill(n, paths.to_vec());
}

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

include!("eof_advance_nav.rs");

/// Shared handles for leaving playback and repainting the recent grid (Escape path).
struct BackToBrowseCtx {
    /// Bottom-bar close (`app.close-video`); tooltip + enable state via [sync_close_video_action].
    close_video_btn: gtk::Button,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    win_aspect: Rc<WinAspectCell>,
    /// Show bars; cancel auto-hide. Call after [gtk::Widget::set_visible] for the browse overlay.
    on_browse: Rc<dyn Fn()>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    /// Stack of removed/trashed entries, newest at the end; [Undo] pops from the end.
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    /// Mirrors browse-overlay [gtk::Widget::is_visible]; refreshed before pausing
    /// on browse-back so transport can skip unloading the motion filter without racing `notify::visible`.
    recent_visible: Rc<Cell<bool>>,
    /// Resume/duration for continue cards; transport reads this instead of SQLite per tick/hover.
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    /// **True** while the main chrome targets the playing file (grid hidden after [try_load] reveal).
    playback_focus: Rc<Cell<bool>>,
    /// First paint used the browse row (no boot file): keep the strip visible with the Open tile
    /// even when history is empty (`false` for CLI/session boot paths).
    browse_has_strip: bool,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}
