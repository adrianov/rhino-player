
/// Advance to the next sibling only on mpv **natural** end: `eof-reached` or `EndFile` with EOF reason.
/// `sibling_eof_done` allows one `try_load` per logical end; cleared when `eof-reached` becomes false.
#[allow(clippy::too_many_arguments)]
fn maybe_advance_sibling_on_eof(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    seof: &SiblingEofState,
    exit_after_current: &Rc<Cell<bool>>,
    app: &adw::Application,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
    on_start: &Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    on_loaded: Option<Rc<dyn Fn()>>,
    reapply: &VideoReapply60,
) {
    let g = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = g.as_ref() else {
        return;
    };
    if seof.done.get() {
        return;
    }
    if exit_after_current.get() {
        seof.done.set(true);
        drop(g);
        schedule_quit_persist(app, win, player, sub_pref, idle_inhib);
        return;
    }
    let finished = local_file_from_mpv(&pl.mpv).or_else(|| last_path.borrow().clone());
    let Some(finished) = finished else {
        seof.done.set(true);
        return;
    };
    let next = sibling_advance::next_after_eof(&finished);
    let no_sibling = next.is_none();
    drop(g);
    seof.done.set(true);
    if let Some(np) = next {
        let o = LoadOpts {
            record: true,
            play_on_start: true,
            last_path: Rc::clone(last_path),
            on_start: Some(Rc::clone(on_start)),
            win_aspect: Rc::clone(&win_aspect),
            on_loaded: on_loaded.as_ref().map(Rc::clone),
            reapply_60: Some(reapply.clone()),
            reset_speed_to_normal: true,
        };
        if let Err(e) = try_load(&np, player, win, gl, recent, &o) {
            eprintln!("[rhino] sibling advance: {e}");
            seof.done.set(false);
        }
    } else if no_sibling {
        // [try_load] only runs on a path change; with no follow-up file, EOF still left the
        // title in the continue list and DB — drop both here.
        remove_continue_entry(&finished);
    }
}

/// Bottom-bar **Previous** / **Next** tooltips: the **file name** of the target in folder/sibling
/// order; [can] is from [SiblingEofState::nav_sensitivity].
fn sibling_bar_tooltip(is_prev: bool, can: bool, cur: Option<&Path>) -> String {
    if !can {
        return if is_prev {
            "No previous file in folder order".to_string()
        } else {
            "No next file in folder order".to_string()
        };
    }
    let Some(c) = cur else {
        return if is_prev {
            "Open previous in folder order".to_string()
        } else {
            "Open next in folder order".to_string()
        };
    };
    let t = if is_prev {
        sibling_advance::prev_before_current(c)
    } else {
        sibling_advance::next_after_eof(c)
    };
    let Some(t) = t else {
        // Rare if [can] and [cur] match [nav_sensitivity]; keep a neutral line if paths diverge.
        return if is_prev {
            "Previous in folder order".to_string()
        } else {
            "Next in folder order".to_string()
        };
    };
    // File name only (non-utf8: lossy); icon shows previous vs next.
    t.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| t.to_string_lossy().into_owned())
}

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
    recent: &gtk::ScrolledWindow,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    let r: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if r.is_empty() {
        recent.set_visible(false);
        return;
    }
    recent.set_visible(true);
    let v: Vec<CardData> = card_data_list(&r);
    recent_view::fill_row(row, v, on_open.clone(), on_remove.clone(), on_trash.clone());
    let n = recent_view::ensure_recent_backfill(rbf, row, on_open, on_remove, on_trash);
    recent_view::schedule_thumb_backfill(n, r);
}

fn cancel_undo_timer(src: &RefCell<Option<glib::source::SourceId>>) {
    if let Some(id) = src.borrow_mut().take() {
        id.remove();
    }
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
    match n {
        1 => btn.set_tooltip_text(Some(
            "Undo: put the file back on the list with prior resume/cache, or restore from trash when the last action was trash.",
        )),
        n => {
            let s = format!(
                "Restores the most recent action. {n} step(s) on the stack (one per click, newest first)."
            );
            btn.set_tooltip_text(Some(s.as_str()));
        }
    }
    if let Some(p) = stack.borrow().last() {
        let (name, tail) = match p {
            ContinueBarUndo::ListRemove(u) => (
                u.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "removed from continue list",
            ),
            ContinueBarUndo::Trash { snap, .. } => (
                snap.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "moved to trash",
            ),
        };
        let line = format!("\u{201c}{name}\u{201d} {tail}");
        label.set_label(&line);
    }
}

fn rearm_undo_dismiss(
    do_commit: &Rc<dyn Fn() + 'static>,
    undo_source: &RefCell<Option<glib::source::SourceId>>,
) {
    cancel_undo_timer(undo_source);
    let c = do_commit.clone();
    *undo_source.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
        c();
        glib::ControlFlow::Break
    }));
}

/// Shared handles for leaving playback and repainting the recent grid (Escape path).
struct BackToBrowseCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Show bars; cancel auto-hide. Call after [gtk::ScrolledWindow::set_visible] for the grid.
    on_browse: Rc<dyn Fn()>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    /// Stack of removed/trashed entries, newest at the end; [Undo] pops from the end.
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
}

