struct WindowInputCtx {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    /// Wraps `root` so overlay children appear above the ToolbarView bottom bar.
    outer_ovl: gtk::Overlay,
    ovl: gtk::Overlay,
    bottom: gtk::Box,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
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
    /// Single closure replacing repeated [BackToBrowseCtx] construction; arg = `clear_undo`.
    on_browse_back: Rc<dyn Fn(bool)>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    play_pause: gtk::Button,
}

fn wire_window_input(ctx: WindowInputCtx) {
    w_in_set_shell(&ctx);
    w_in_fullscreen(&ctx);
    w_in_max_mode(&ctx);
    w_in_win_motion(&ctx);
    w_in_gl_motion(&ctx);
    w_in_key_controller(&ctx);
}
