fn nudge_mpv_volume(mpv: &Mpv, delta: f64) {
    let nv = (mpv.get_property::<f64>("volume").unwrap_or(0.0) + delta).clamp(
        0.0,
        mpv
            .get_property::<f64>("volume-max")
            .unwrap_or(100.0)
            .max(1.0),
    );
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
    recent.set_visible(true);
    repaint_continue_row(row, rbf, &[], &on_open, &on_remove, &on_trash, &chrome_cache);
}

/// Repaint via [recent_view::ensure_apply_strip]. `_paths` kept for call-site stability;
/// the strip source is query-aware inside [RecentContext::apply_strip].
fn repaint_continue_row(
    row: &gtk::Box,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
    _paths: &[PathBuf],
    on_open: &RcPathFn,
    on_remove: &RcPathFn,
    on_trash: &RcPathFn,
    chrome_cache: &crate::media_probe::ContinueGridCache,
) {
    recent_view::ensure_apply_strip(
        rbf,
        row,
        recent_view::strip_hooks_from_cell(
            rbf,
            Rc::clone(on_open),
            Rc::clone(on_remove),
            Rc::clone(on_trash),
            Rc::clone(chrome_cache),
        ),
    );
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
