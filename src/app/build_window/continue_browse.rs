include!("browse_chrome_hover.rs");

/// Continue-grid session: open-fail toast, warm hover, undo snackbar, Escape back-to-browse.
struct ContinueBrowse {
    on_open_fail: Rc<dyn Fn(String)>,
    on_browse_back: Rc<dyn Fn(bool)>,
    recent_visible: Rc<Cell<bool>>,
    warm_preload: Option<Rc<WarmPreloadCtx>>,
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    undo_timer: Rc<RefCell<Option<glib::SourceId>>>,
    do_commit: Rc<dyn Fn()>,
}

struct ContinueBrowsePrep {
    on_open_fail: Rc<dyn Fn(String)>,
    browse_back_slot: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>,
}

struct ContinueBrowseFinish<'a> {
    want_recent: bool,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &'a Rc<RefCell<db::VideoPrefs>>,
    w: &'a WindowWidgets,
    last_path: &'a Rc<RefCell<Option<PathBuf>>>,
    on_open: RcPathFn,
    sibling_seof: &'a Rc<SiblingEofState>,
    win_aspect: &'a Rc<WinAspectCell>,
    playback_focus: &'a Rc<Cell<bool>>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    dvd_bar: &'a Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    nav_t: &'a Rc<RefCell<Option<glib::SourceId>>>,
    bar_show: &'a Rc<Cell<bool>>,
}

impl ContinueBrowsePrep {
    fn start(
        notice: Rc<crate::recent_view::NoticeToastCtrl>,
        recent: gtk::Box,
        playback_focus: Rc<Cell<bool>>,
    ) -> Self {
        let browse_back_slot: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
        let on_open_fail: Rc<dyn Fn(String)> = {
            let notice = Rc::clone(&notice);
            let recent = recent.clone();
            let slot = Rc::clone(&browse_back_slot);
            let playback_focus = Rc::clone(&playback_focus);
            Rc::new(move |msg: String| {
                // Notice lives under the continue strip. Return to browse when playback UI is up.
                if !recent.is_visible() || playback_focus.get() {
                    Self::return_to_browse(&slot, &recent, &playback_focus);
                }
                notice.show(&msg);
            })
        };
        Self {
            on_open_fail,
            browse_back_slot,
        }
    }

    fn return_to_browse(
        slot: &RefCell<Option<Rc<dyn Fn(bool)>>>,
        recent: &gtk::Box,
        playback_focus: &Cell<bool>,
    ) {
        if let Some(bb) = slot.borrow().as_ref() {
            bb(false);
        }
        recent.set_visible(true);
        playback_focus.set(false);
    }

    fn finish(self, f: ContinueBrowseFinish<'_>) -> ContinueBrowse {
        let browse_chrome = rc_on_browse_chrome(BrowseChromeRefs {
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
        });
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
        let continue_grid_cache = Rc::new(RefCell::new(std::collections::HashMap::new()));
        crate::media_probe::continue_grid_cache_attach(Rc::clone(&continue_grid_cache));
        let recent_wiring = wire_recent_undo(RecentUndoCtx {
            player: f.player.clone(),
            recent: f.w.recent_scrl.clone(),
            flow: f.w.flow_recent.clone(),
            undo_shell: f.w.undo_bar.shell.clone(),
            undo_label: f.w.undo_bar.label.clone(),
            undo_btn: f.w.undo_bar.undo.clone(),
            undo_close: f.w.undo_bar.close.clone(),
            on_open: f.on_open.clone(),
            want_recent: f.want_recent,
            warm_hover: warm_hover.clone(),
            continue_grid_cache: Rc::clone(&continue_grid_cache),
        });
        // `is_visible()` is false until the window is mapped; use `want_recent` so transport
        // and warm-preload see the continue strip on empty launch before `present`.
        let recent_visible = Rc::new(Cell::new(f.want_recent));
        {
            let rv = Rc::clone(&recent_visible);
            f.w.recent_scrl
                .connect_notify_local(Some("visible"), move |r, _| rv.set(r.is_visible()));
        }
        let on_browse_back = make_browse_back(
            BackToBrowseCtx {
                player: f.player.clone(),
                close_video_btn: f.w.close_video_btn.clone(),
                close_action_cell: Rc::clone(&f.close_action_cell),
                on_open: f.on_open.clone(),
                on_remove: recent_wiring.on_remove.clone(),
                on_trash: recent_wiring.on_trash.clone(),
                recent_backfill: recent_wiring.recent_backfill.clone(),
                last_path: f.last_path.clone(),
                sibling_seof: f.sibling_seof.clone(),
                sibling_nav: f.w.sibling_nav.clone(),
                win_aspect: f.win_aspect.clone(),
                on_browse: browse_chrome,
                undo_shell: f.w.undo_bar.shell.clone(),
                undo_label: f.w.undo_bar.label.clone(),
                undo_btn: f.w.undo_bar.undo.clone(),
                undo_timer: recent_wiring.undo_timer.clone(),
                undo_remove_stack: recent_wiring.undo_remove_stack.clone(),
                recent_visible: Rc::clone(&recent_visible),
                playback_focus: Rc::clone(f.playback_focus),
                browse_has_strip: true,
                hdr_title_mirror: f.w.hdr_title_mirror.clone(),
                continue_grid_cache: Rc::clone(&continue_grid_cache),
                dvd_bar: Rc::clone(f.dvd_bar),
            },
            f.w.win.clone(),
            f.w.gl_area.clone(),
            f.w.recent_scrl.clone(),
            f.w.flow_recent.clone(),
        );
        *self.browse_back_slot.borrow_mut() = Some(Rc::clone(&on_browse_back));
        ContinueBrowse {
            on_open_fail: self.on_open_fail,
            on_browse_back,
            recent_visible,
            warm_preload,
            continue_grid_cache,
            pending_recent_backfill: recent_wiring.pending_recent_backfill,
            undo_remove_stack: recent_wiring.undo_remove_stack,
            undo_timer: recent_wiring.undo_timer,
            do_commit: recent_wiring.do_commit,
        }
    }
}
