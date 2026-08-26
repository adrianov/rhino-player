include!("browse_chrome_hover.rs");

include!("continue_browse_finish.rs");

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

type BrowseBackSlot = Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>;

struct ContinueBrowsePrep {
    on_open_fail: Rc<dyn Fn(String)>,
    browse_back_slot: BrowseBackSlot,
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
        let browse_back_slot: BrowseBackSlot = Rc::new(RefCell::new(None));
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

    fn return_to_browse(slot: &BrowseBackSlot, recent: &gtk::Box, playback_focus: &Cell<bool>) {
        if let Some(bb) = slot.borrow().as_ref() {
            bb(false);
        }
        recent.set_visible(true);
        playback_focus.set(false);
    }

    fn finish(self, f: ContinueBrowseFinish<'_>) -> ContinueBrowse {
        let browse_chrome = finish_browse_chrome(&f);
        let (warm_preload, warm_hover) = finish_warm_preload(&f);
        let continue_grid_cache = new_continue_grid_cache();
        let recent_wiring = finish_recent_undo_wiring(&f, warm_hover, &continue_grid_cache);
        let recent_visible = finish_track_recent_visible(&f);
        let on_browse_back = finish_on_browse_back(
            &f,
            browse_chrome,
            &recent_wiring,
            &recent_visible,
            &continue_grid_cache,
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
