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
    // Query-aware strip source: neighbour-substring results replace the plain list while a
    // search is active (feature 33). Thumbnail backfill below keeps targeting history entries.
    let ctx = rbf.borrow().clone();
    let plan = recent_view::strip_plan(search_of(&ctx), paths.to_vec());
    if let Some(c) = &ctx {
        c.paint(plan.paths.clone(), plan.kind);
    }
    if plan.searching {
        if let Some(c) = &ctx {
            c.note_search_hint();
        }
    }
    // Thumbnail backfill keeps targeting the real watch-later entries even while results
    // replace them visually; `paths` is always the history slice.
    backfill_continue_row(rbf, row, paths, on_open, on_remove, on_trash, chrome_cache);
}

/// Shared search state borrowed out of an optional strip context.
fn search_of(ctx: &Option<Rc<RecentContext>>) -> Option<&recent_view::SiblingSearchState> {
    ctx.as_ref().and_then(|c| c.search.as_deref())
}

/// Continue-strip hooks derived from the row's context: warm-hover hooks and the shared
/// neighbour-search state ride along with whichever context painted last.
fn strip_hooks(
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    on_open: &RcPathFn,
    on_remove: &RcPathFn,
    on_trash: &RcPathFn,
    chrome_cache: &crate::media_probe::ContinueGridCache,
) -> recent_view::ContinueStripHooks {
    let ctx = rbf.borrow();
    recent_view::ContinueStripHooks {
        on_open: Rc::clone(on_open),
        on_remove: Rc::clone(on_remove),
        on_trash: Rc::clone(on_trash),
        warm_hover: ctx.as_ref().and_then(|c| c.warm_hover().cloned()),
        chrome_cache: Rc::clone(chrome_cache),
        search: ctx.as_ref().and_then(|c| c.search.as_ref().map(Rc::clone)),
    }
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
    let n = recent_view::ensure_recent_backfill(
        rbf,
        row,
        strip_hooks(rbf, on_open, on_remove, on_trash, chrome_cache),
    );
    recent_view::schedule_thumb_backfill(n, paths.to_vec());
}

include!("eof_advance_nav.rs");
include!("undo_bar_presentation.rs");

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
