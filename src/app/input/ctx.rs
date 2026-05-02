/// Main window widgets referenced by capture-phase motion and shell layout.
struct WindowInputShell {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    outer_ovl: gtk::Overlay,
    video_handle: gtk::WindowHandle,
    bottom: gtk::Box,
    gl: gtk::GLArea,
    recent: gtk::Box,
}

struct WindowInputCtx {
    shell: WindowInputShell,
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    bar_show: Rc<Cell<bool>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
    cur_t: Rc<RefCell<Option<glib::SourceId>>>,
    ptr_in_gl: Rc<Cell<bool>>,
    motion_squelch: Rc<Cell<Option<Instant>>>,
    last_cap_xy: Rc<Cell<Option<(f64, f64)>>>,
    last_gl_xy: Rc<Cell<Option<(f64, f64)>>>,
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    ch_hide: Rc<ChromeBarHide>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    /// Single closure replacing repeated [BackToBrowseCtx] construction; arg = `clear_undo`.
    on_browse_back: Rc<dyn Fn(bool)>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    sibling_seof: Rc<SiblingEofState>,
    play_pause: gtk::Button,
    seek: gtk::Scale,
    seek_sync: Rc<Cell<bool>>,
    time_left: gtk::Label,
    fs_clock: gtk::Label,
    fs_clock_tick: Rc<RefCell<Option<glib::SourceId>>>,
    reapply_60: VideoReapply60,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    speed_sync: Rc<Cell<bool>>,
    speed_list: gtk::ListBox,
    speed_readout: gtk::Label,
}

fn wire_window_input(ctx: WindowInputCtx) {
    w_in_set_shell(&ctx);
    w_in_fullscreen(&ctx);
    w_in_max_mode(&ctx);
    w_in_win_motion(&ctx);
    w_in_gl_motion(&ctx);
    w_in_key_controller(&ctx);
}
