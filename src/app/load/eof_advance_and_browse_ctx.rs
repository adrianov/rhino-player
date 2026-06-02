
/// Advance to the next sibling only on mpv **natural** end: `eof-reached` or `EndFile` with EOF reason.
/// `sibling_eof_done` allows one `try_load` per logical end; cleared when `eof-reached` becomes false.
#[allow(clippy::too_many_arguments)]
fn maybe_advance_sibling_on_eof(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::Box,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    seof: &SiblingEofState,
    exit_after_current: &Rc<Cell<bool>>,
    app: &adw::Application,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    idle_inhib: &Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    teardown_after_draw: &Rc<Cell<bool>>,
    on_start: &Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    on_loaded: Option<Rc<dyn Fn()>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
) {
    let g = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = g.as_ref() else {
        return;
    };
    // Continue grid / warm hover: paused preload only — no sibling auto-advance (would call try_load with play).
    if crate::app::browse_overlay_active(recent) {
        return;
    }
    if seof.done.get() {
        return;
    }
    if exit_after_current.get() {
        seof.done.set(true);
        drop(g);
        schedule_quit_persist(app, win, gl, player, sub_pref, idle_inhib, teardown_after_draw);
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
        if crate::video_ext::paths_same_file(&np, &finished) {
            return;
        }
        let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
            video_pref: Rc::clone(video_pref),
            last_path: Rc::clone(last_path),
            on_start: Some(Rc::clone(on_start)),
            win_aspect: Rc::clone(&win_aspect),
            on_loaded: on_loaded.as_ref().map(Rc::clone),
            play_on_start: true,
            reset_speed_to_normal: true,
            hdr_title_mirror,
        });
        o.playback_focus = Some(Rc::clone(&playback_focus));
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

/// Bottom-bar **Previous** / **Next** tooltips: humanized **base name** of the target in folder/sibling
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
    // Lossy UTF-8 from `OsStr`; humanized like window title / continue cards.
    let label_path = crate::video_ext::dvd_disc_root(&t).unwrap_or_else(|| t.to_path_buf());
    let raw = label_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| label_path.to_string_lossy().into_owned());
    crate::human_media_title::human_media_title(&raw)
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
    let v: Vec<CardData> = card_data_list(&r);
    let warm = rbf
        .borrow()
        .as_ref()
        .and_then(|c| c.warm_hover().cloned());
    recent_view::fill_row(
        row,
        v,
        on_open.clone(),
        on_remove.clone(),
        on_trash.clone(),
        warm.as_ref(),
        Some(&chrome_cache),
    );
    let warm_ctx = rbf.borrow().as_ref().and_then(|c| c.warm_hover().cloned());
    let n = recent_view::ensure_recent_backfill(
        rbf,
        row,
        on_open,
        on_remove,
        on_trash,
        warm_ctx,
        Rc::clone(&chrome_cache),
    );
    recent_view::schedule_thumb_backfill(n, r);
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

