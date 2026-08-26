include!("wire_handlers_before_mpv_phases.rs");
include!("wire_handlers_before_mpv_loaded.rs");

struct HandlersBeforeMpv {
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    ch_hide: Rc<ChromeBarHide>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    play_ctx: PlayToggleCtx,
    on_browse_back: Rc<dyn Fn(bool)>,
    pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
    undo_timer: Rc<RefCell<Option<glib::SourceId>>>,
    do_commit: Rc<dyn Fn()>,
    recent_visible: Rc<Cell<bool>>,
    close_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    trash_action_cell: Rc<RefCell<Option<gio::SimpleAction>>>,
    warm_preload: Option<Rc<WarmPreloadCtx>>,
    on_open_fail: Rc<dyn Fn(String)>,
}

/// Long-lived state refs handed to the pre-MPV wiring (grouped from the former flat list).
struct BeforeMpvRefs<'a> {
    app: &'a adw::Application,
    w: &'a WindowWidgets,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &'a Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: &'a Rc<RefCell<Option<RcPathFn>>>,
    prefs: &'a BwPrefsState,
    tl: &'a BwTimelineState,
    chrome: &'a BwChromeState,
}

fn wire_handlers_before_mpv(r: BeforeMpvRefs<'_>) -> HandlersBeforeMpv {
    let h = wire_pre_mpv_phases(PreMpvPhaseRefs {
        app: r.app,
        w: r.w,
        player: r.player,
        file_boot: r.file_boot,
        on_open_slot: r.on_open_slot,
        sub_pref: &r.prefs.sub_pref,
        video_pref: &r.prefs.video_pref,
        bar_show: &r.tl.bar_show,
        nav_t: &r.tl.nav_t,
        motion_squelch: &r.tl.motion_squelch,
        playback_focus: &r.tl.playback_focus,
        win_aspect: &r.chrome.win_aspect,
        last_path: &r.tl.last_path,
        sibling_seof: &r.tl.sibling_seof,
        dvd_bar: &r.tl.dvd_bar,
        fs_restore: &r.chrome.fs_restore,
        last_unmax: &r.chrome.last_unmax,
        skip_max_to_fs: &r.chrome.skip_max_to_fs,
        fs_transition_busy: &r.chrome.fs_transition_busy,
    });
    HandlersBeforeMpv {
        continue_grid_cache: h.browse.continue_grid_cache,
        seek_sync: h.seek_sync,
        seek_grabbed: h.seek_grabbed,
        smooth_seek_debounce: h.smooth_seek_debounce,
        resume_after_seek_idle: h.resume_after_seek_idle,
        hdr_csd_baseline: h.vc.hdr_csd_baseline,
        ch_hide: h.vc.ch_hide,
        on_video_chrome: h.vc.on_show,
        on_file_loaded: h.on_file_loaded,
        play_ctx: h.media_open.play_ctx,
        on_browse_back: h.browse.on_browse_back,
        pending_recent_backfill: h.browse.pending_recent_backfill,
        undo_remove_stack: h.browse.undo_remove_stack,
        undo_timer: h.browse.undo_timer,
        do_commit: h.browse.do_commit,
        recent_visible: h.browse.recent_visible,
        close_action_cell: h.close_action_cell,
        trash_action_cell: h.trash_action_cell,
        warm_preload: h.browse.warm_preload,
        on_open_fail: h.browse.on_open_fail,
    }
}
