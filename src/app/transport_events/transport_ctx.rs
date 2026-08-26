// Shared transport wiring state: widget handles, mpv-derived cache, and the context bundle
// threaded through every transport tick / event dispatcher.

struct TransportWidgets {
    play_pause: gtk::Button,
    seek: gtk::Scale,
    seek_adj: gtk::Adjustment,
    seek_sync: Rc<Cell<bool>>,
    /// True while the user is pressing the seek thumb (mouse / touch). The 1 Hz tick skips
    /// programmatic position writes so dragging the thumb is not interrupted.
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    time_right: gtk::Label,
    speed_menu: gtk::MenuButton,
    speed_readout: gtk::Label,
    vol_menu: gtk::MenuButton,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_adj: gtk::Adjustment,
    vol_mute: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
    sub_readout: gtk::Label,
    smooth_toolbar_btn: gtk::Button,
    smooth_toolbar_status: gtk::Label,
}

#[derive(Default)]
struct TransportCache {
    duration: f64,
    pause: bool,
    pos: f64,
    /// True when mpv playback core is not progressing (EOF with `keep-open=yes`, buffering, seeking, stalled).
    core_idle: bool,
}

struct TransportEofCtx {
    app: adw::Application,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<WinAspectCell>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: Rc<dyn Fn(String)>,
}

struct TransportCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    widgets: TransportWidgets,
    eof: TransportEofCtx,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    smooth_budget_decoder: Rc<RefCell<crate::video_pref::SmoothBudgetDecoderState>>,
    /// Bottom-bar visibility flag; transient seek-slider redraws are skipped while it is `false`
    /// to avoid invalidating chrome that is animating in / out (the cause of fullscreen flicker).
    bar_show: Rc<Cell<bool>>,
    /// Toggled to keep the recent grid path in sync; if `recent` is visible the seek bar is hidden too.
    recent_visible: Rc<Cell<bool>>,
    sibling_nav: SiblingNavUi,
    /// Coalesce [glib::idle_add_local_once] resyncs on `FileLoaded` / `path` churn.
    idle_resync_pending: Rc<Cell<bool>>,
    /// Debounced [glib::timeout_add_local] after `FileLoaded` / `VideoReconfig` / `path` / `container-fps`
    /// so one [smooth_60_full_resync_after_media_change] runs when the burst settles.
    smooth_60_resync_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// 1 Hz timer source id (kept so it can be replaced if observers re-install).
    tick: Rc<RefCell<Option<glib::SourceId>>>,
    cache: Rc<RefCell<TransportCache>>,
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
    /// Stable DVD title bar range (SQLite chapter lengths); not recomputed every transport tick.
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    blackout: Rc<crate::screen_blackout::BlackoutSync>,
}
