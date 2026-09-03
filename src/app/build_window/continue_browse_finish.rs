/// Chrome callback used when returning to Browse after playback (Escape strip).
fn finish_browse_chrome(f: &ContinueBrowseFinish<'_>) -> Rc<dyn Fn()> {
    rc_on_browse_chrome(BrowseChromeRefs {
        hdr_csd: Rc::clone(&f.hdr_csd_baseline),
        nav_t: f.nav_t.clone(),
        win: f.w.win.clone(),
        root: f.w.root.clone(),
        gl: f.w.gl_area.clone(),
        bar_show: f.bar_show.clone(),
        recent: f.w.recent_scrl.clone(),
        bottom: f.w.bottom.clone(),
        player: f.player.clone(),
        header: f.w.header.clone(),
    })
}

/// Warm-preload context and its hover hooks (continue-strip launches only).
fn finish_warm_preload(
    f: &ContinueBrowseFinish<'_>,
) -> (
    Option<Rc<WarmPreloadCtx>>,
    Option<crate::recent_view::WarmHoverHooks>,
) {
    let warm_preload = f.want_recent.then(|| {
        WarmPreloadCtx::new(
            f.player.clone(),
            Rc::clone(f.video_pref),
            f.w.recent_scrl.clone(),
            f.w.gl_area.clone(),
            f.last_path.clone(),
        )
    });
    let warm_hover = warm_preload
        .as_ref()
        .map(|ctx| warm_hover_hooks(Rc::clone(ctx)));
    (warm_preload, warm_hover)
}

fn new_continue_grid_cache() -> crate::media_probe::ContinueGridCache {
    let cache = Rc::new(RefCell::new(std::collections::HashMap::new()));
    crate::media_probe::continue_grid_cache_attach(Rc::clone(&cache));
    cache
}

/// Undo snackbar + remove/trash/backfill wiring for the continue grid.
fn finish_recent_undo_wiring(
    f: &ContinueBrowseFinish<'_>,
    warm_hover: Option<crate::recent_view::WarmHoverHooks>,
    continue_grid_cache: &crate::media_probe::ContinueGridCache,
) -> RecentUndoWiring {
    wire_recent_undo(RecentUndoCtx {
        recent: f.w.recent_scrl.clone(),
        flow: f.w.flow_recent.clone(),
        undo_shell: f.w.undo_bar.shell.clone(),
        undo_label: f.w.undo_bar.label.clone(),
        undo_btn: f.w.undo_bar.undo.clone(),
        undo_close: f.w.undo_bar.close.clone(),
        on_open: f.on_open.clone(),
        want_recent: f.want_recent,
        warm_hover,
        continue_grid_cache: Rc::clone(continue_grid_cache),
        search: Some(f.w.sibling_search.shared()),
    })
}

/// Mirrors strip visibility into a cell; hides the search row when the strip hides.
fn finish_track_recent_visible(f: &ContinueBrowseFinish<'_>) -> Rc<Cell<bool>> {
    // `is_visible()` is false until the window is mapped; seed from `want_recent` so
    // transport and warm-preload see the continue strip on empty launch before `present`.
    let recent_visible = Rc::new(Cell::new(f.want_recent));
    let search = f.w.sibling_search.shared();
    search.bind_strip_hide();
    search.sync_browse_visible(f.want_recent);
    {
        let rv = Rc::clone(&recent_visible);
        f.w.recent_scrl
            .connect_notify_local(Some("visible"), move |r, _| {
                let vis = r.is_visible();
                rv.set(vis);
                search.sync_browse_visible(vis);
                if vis {
                    crate::seek_bar_preview::dismiss_for_browse();
                }
            });
    }
    recent_visible
}

type BrowseBackRefsA = (
    Rc<RefCell<Option<MpvBundle>>>,
    gtk::Button,
    Rc<RefCell<Option<gio::SimpleAction>>>,
    RcPathFn,
    RcPathFn,
    RcPathFn,
    Rc<RefCell<Option<Rc<RecentContext>>>>,
);

fn browse_back_refs_a(f: &ContinueBrowseFinish<'_>, w: &RecentUndoWiring) -> BrowseBackRefsA {
    (
        f.player.clone(),
        f.w.close_video_btn.clone(),
        f.close_action_cell.clone(),
        f.on_open.clone(),
        w.on_remove.clone(),
        w.on_trash.clone(),
        w.recent_backfill.clone(),
    )
}

type BrowseBackRefsB = (
    Rc<RefCell<Option<PathBuf>>>,
    Rc<SiblingEofState>,
    SiblingNavUi,
    Rc<WinAspectCell>,
    Rc<RefCell<Option<glib::source::SourceId>>>,
    Rc<RefCell<Vec<ContinueBarUndo>>>,
    Option<Rc<gtk::Label>>,
    Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    gtk::Box,
    gtk::Label,
    gtk::Button,
);

fn browse_back_refs_b(f: &ContinueBrowseFinish<'_>, w: &RecentUndoWiring) -> BrowseBackRefsB {
    (
        f.last_path.clone(),
        f.sibling_seof.clone(),
        f.w.sibling_nav.clone(),
        f.win_aspect.clone(),
        w.undo_timer.clone(),
        w.undo_remove_stack.clone(),
        f.w.hdr_title_mirror.clone(),
        f.dvd_bar.clone(),
        f.w.undo_bar.shell.clone(),
        f.w.undo_bar.label.clone(),
        f.w.undo_bar.undo.clone(),
    )
}

/// Assemble the Escape/browse-back closure over the shared handles.
fn finish_on_browse_back(
    f: &ContinueBrowseFinish<'_>,
    browse_chrome: Rc<dyn Fn()>,
    recent_wiring: &RecentUndoWiring,
    recent_visible: &Rc<Cell<bool>>,
    continue_grid_cache: &crate::media_probe::ContinueGridCache,
) -> Rc<dyn Fn(bool)> {
    // Indexed tuple access keeps this assembly under the AbcSize assignment budget.
    let ra = browse_back_refs_a(f, recent_wiring);
    let rb = browse_back_refs_b(f, recent_wiring);
    make_browse_back(
        BackToBrowseCtx {
            player: ra.0,
            close_video_btn: ra.1,
            close_action_cell: ra.2,
            on_open: ra.3,
            on_remove: ra.4,
            on_trash: ra.5,
            recent_backfill: ra.6,
            last_path: rb.0,
            sibling_seof: rb.1,
            sibling_nav: rb.2,
            win_aspect: rb.3,
            on_browse: browse_chrome,
            undo_shell: rb.8,
            undo_label: rb.9,
            undo_btn: rb.10,
            undo_timer: rb.4,
            undo_remove_stack: rb.5,
            recent_visible: Rc::clone(recent_visible),
            playback_focus: Rc::clone(f.playback_focus),
            browse_has_strip: true,
            hdr_title_mirror: rb.6,
            continue_grid_cache: Rc::clone(continue_grid_cache),
            dvd_bar: rb.7,
        },
        f.w.win.clone(),
        f.w.gl_area.clone(),
        f.w.recent_scrl.clone(),
        f.w.flow_recent.clone(),
    )
}
