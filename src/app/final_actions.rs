struct FinalActionCtx {
    app: adw::Application,
    win: adw::ApplicationWindow,
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    fs_transition_busy: Rc<Cell<bool>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    gl: gtk::GLArea,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    /// **True** while the playing layout is active ([try_load] hid the browse grid).
    playback_focus: Rc<Cell<bool>>,
    /// macOS global menu bar model (File / View).
    #[cfg(target_os = "macos")]
    main_menu: gio::Menu,
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    bar_show: Rc<Cell<bool>>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    exit_after_current: Rc<Cell<bool>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    smooth_toolbar_status: gtk::Label,
}

include!("final_actions_smooth_resize.rs");

include!("final_actions_wire.rs");

fn wire_final_actions(ctx: FinalActionCtx) {
    wire_final_open_dialog(&ctx);
    wire_final_about_dialog(&ctx);
    wire_quit_close(
        &ctx.app,
        &ctx.win,
        &ctx.gl,
        &ctx.player,
        &ctx.sub_pref,
        &ctx.idle_inhib,
        &ctx.mpv_teardown_after_draw,
    );
    wire_final_exit_after_toggle(&ctx);
    wire_final_fullscreen_toggle(&ctx);
    register_video_app_actions(
        &ctx.app,
        &ctx.win,
        &ctx.gl,
        &ctx.player,
        Rc::clone(&ctx.video_pref),
        VideoAppMenuWire {
            pref_menu: ctx.pref_menu.clone(),
            seek_bar_on: Rc::clone(&ctx.seek_bar_on),
            smooth_toolbar_status: Some(ctx.smooth_toolbar_status.clone()),
        },
    );
    wire_final_platform_accels(&ctx);
    wire_final_idle_chrome_resize(&ctx);
    ctx.win.present();
}

include!("final_actions_quit_close.rs");
