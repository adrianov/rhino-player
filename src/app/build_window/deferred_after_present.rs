include!("deferred_after_present/seek_preview_and_control.rs");
include!("deferred_after_present/input_and_remote.rs");
include!("deferred_after_present/macos_remote.rs");
include!("deferred_after_present/mpris_volume.rs");
include!("deferred_after_present/transport_wire.rs");
include!("deferred_after_present/final_actions_wire.rs");
include!("deferred_after_present/warm_preload_register.rs");

thread_local! {
    /// Set before [present]; consumed when the mpv GL bundle is created (same idle turn).
    static AFTER_PRESENT_ARGS: RefCell<Option<WindowAfterPresentArgs>> = const { RefCell::new(None) };
}

fn stash_after_present_args(args: WindowAfterPresentArgs) {
    AFTER_PRESENT_ARGS.with(|s| *s.borrow_mut() = Some(args));
}

fn run_stashed_after_present_wire() {
    let args = AFTER_PRESENT_ARGS.with(|s| s.borrow_mut().take());
    if let Some(args) = args {
        wire_window_after_present(args);
    }
}

/// Input / transport / menus — runs once the mpv bundle exists (from the realize idle).
/// Each wiring concern lives in one step fn; see `deferred_after_present/*`.
fn wire_window_after_present(args: WindowAfterPresentArgs) {
    let (preview_hover_t, preview_player) = connect_seek_preview_after_present(&args);
    wire_window_input_step(&args);
    #[cfg(target_os = "macos")]
    wire_macos_now_playing_step(&args);
    wire_seek_control_step(&args, preview_hover_t, preview_player);
    #[cfg(target_os = "linux")]
    wire_mpris_linux_step(&args);
    wire_volume_controls_step(&args);
    wire_aspect_resize_on_map(
        &args.w.win,
        &args.w.recent_scrl,
        &args.win_aspect,
        &args.aspect_resize_end_deb,
        &args.aspect_resize_wired,
    );
    wire_transport_events_step(&args);
    wire_final_actions_step(&args);
    register_warm_preload_step(args);
}

struct WindowAfterPresentArgs {
    app: adw::Application,
    w: WindowWidgets,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    seek_bar_on: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    bar_show: Rc<Cell<bool>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
    cur_t: Rc<RefCell<Option<glib::SourceId>>>,
    ptr_in_gl: Rc<Cell<bool>>,
    motion_squelch: Rc<Cell<Option<Instant>>>,
    last_cap_xy: Rc<Cell<Option<(f64, f64)>>>,
    last_gl_xy: Rc<Cell<Option<(f64, f64)>>>,
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    fs_pause_stash: Rc<RefCell<Option<bool>>>,
    fs_transition_busy: Rc<Cell<bool>>,
    fs_transition_settle: Rc<RefCell<Option<glib::SourceId>>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    ch_hide: Rc<ChromeBarHide>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    on_browse_back: Rc<dyn Fn(bool)>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    sibling_seof: Rc<SiblingEofState>,
    playback_focus: Rc<Cell<bool>>,
    play_ctx: PlayToggleCtx,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    exit_after_current: Rc<Cell<bool>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    reapply_60: VideoReapply60,
    recent_visible: Rc<Cell<bool>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    vol_sync: Rc<Cell<bool>>,
    aspect_resize_end_deb: Rc<RefCell<Option<glib::SourceId>>>,
    aspect_resize_wired: Rc<Cell<bool>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    warm_preload: Option<Rc<WarmPreloadCtx>>,
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    on_open_fail: Rc<dyn Fn(String)>,
}
