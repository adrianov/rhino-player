/// Cloned state handles, group A (indexed tuples keep the ctx assembly under AbcSize budgets).
type WapInputStateA = (
    Rc<RefCell<Option<MpvBundle>>>,
    Rc<RefCell<db::VideoPrefs>>,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<Cell<bool>>,
    Rc<Cell<Option<Instant>>>,
    Rc<Cell<Option<(f64, f64)>>>,
    Rc<Cell<Option<(f64, f64)>>>,
    Rc<RefCell<Option<(i32, i32)>>>,
);

fn wap_input_state_a(args: &WindowAfterPresentArgs) -> WapInputStateA {
    (
        args.player.clone(),
        args.video_pref.clone(),
        args.bar_show.clone(),
        args.nav_t.clone(),
        args.cur_t.clone(),
        args.ptr_in_gl.clone(),
        args.motion_squelch.clone(),
        args.last_cap_xy.clone(),
        args.last_gl_xy.clone(),
        args.fs_restore.clone(),
    )
}

/// Cloned state handles, group B.
type WapInputStateB = (
    Rc<RefCell<Option<bool>>>,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<Cell<bool>>,
    Rc<RefCell<(i32, i32)>>,
    Rc<ChromeBarHide>,
    Rc<Cell<Option<(bool, bool)>>>,
    Rc<dyn Fn(bool)>,
    Rc<dyn Fn()>,
    Rc<dyn Fn()>,
);

fn wap_input_state_b(args: &WindowAfterPresentArgs) -> WapInputStateB {
    (
        args.fs_pause_stash.clone(),
        args.fs_transition_busy.clone(),
        args.fs_transition_settle.clone(),
        args.skip_max_to_fs.clone(),
        args.last_unmax.clone(),
        args.ch_hide.clone(),
        args.hdr_csd_baseline.clone(),
        args.on_browse_back.clone(),
        args.on_video_chrome.clone(),
        args.on_file_loaded.clone(),
    )
}

/// Cloned state handles, group C.
type WapInputStateC = (
    adw::Application,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<WinAspectCell>,
    Rc<SiblingEofState>,
    Rc<Cell<bool>>,
    Rc<dyn Fn(String)>,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<glib::SourceId>>>,
    Rc<Cell<bool>>,
    Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    Option<Rc<gtk::Label>>,
);

fn wap_input_state_c(args: &WindowAfterPresentArgs) -> WapInputStateC {
    (
        args.app.clone(),
        args.last_path.clone(),
        args.win_aspect.clone(),
        args.sibling_seof.clone(),
        args.playback_focus.clone(),
        args.on_open_fail.clone(),
        args.seek_sync.clone(),
        args.smooth_seek_debounce.clone(),
        args.resume_after_seek_idle.clone(),
        args.dvd_bar.clone(),
        args.hdr_title_mirror.clone(),
    )
}

/// Widget handles beyond the input shell (play/pause, clock labels, speed readout).
type WapInputWidgetsX = (
    gtk::Button,
    gtk::Label,
    gtk::Label,
    Rc<Cell<bool>>,
    gtk::Label,
);

fn wap_input_widgets_x(args: &WindowAfterPresentArgs) -> WapInputWidgetsX {
    (
        args.w.play_pause.clone(),
        args.w.time_left.clone(),
        args.w.fs_clock.clone(),
        args.w.speed_sync.clone(),
        args.w.speed_readout.clone(),
    )
}

/// Capture-phase shell widget bundle for [WindowInputCtx].
fn window_input_shell(w: &WindowWidgets) -> WindowInputShell {
    WindowInputShell {
        win: w.win.clone(),
        root: w.root.clone(),
        header: w.header.clone(),
        outer_ovl: w.outer_ovl.clone(),
        video_handle: w.video_handle.clone(),
        bottom: w.bottom.clone(),
        #[cfg(target_os = "macos")]
        bottom_shell: w.bottom_shell.clone(),
        gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(),
    }
}

/// Input / keyboard / motion wiring step of [wire_window_after_present].
fn wire_window_input_step(args: &WindowAfterPresentArgs) {
    let fs_clock_tick = Rc::new(RefCell::new(None::<glib::SourceId>));
    let sa = wap_input_state_a(args);
    let sb = wap_input_state_b(args);
    let sc = wap_input_state_c(args);
    let wx = wap_input_widgets_x(args);
    wire_window_input(WindowInputCtx {
        shell: window_input_shell(&args.w),
        app: sc.0,
        player: sa.0,
        video_pref: sa.1,
        bar_show: sa.2,
        nav_t: sa.3,
        cur_t: sa.4,
        ptr_in_gl: sa.5,
        motion_squelch: sa.6,
        last_cap_xy: sa.7,
        last_gl_xy: sa.8,
        fs_restore: sa.9,
        fs_pause_stash: sb.0,
        fs_transition_busy: sb.1,
        fs_transition_settle: sb.2,
        skip_max_to_fs: sb.3,
        last_unmax: sb.4,
        ch_hide: sb.5,
        hdr_csd_baseline: sb.6,
        on_browse_back: sb.7,
        on_video_chrome: sb.8,
        on_file_loaded: sb.9,
        last_path: sc.1,
        win_aspect: sc.2,
        sibling_seof: sc.3,
        playback_focus: sc.4,
        on_open_fail: sc.5,
        play_pause: wx.0,
        seek: args.w.seek.clone(),
        seek_sync: sc.6,
        time_left: wx.1,
        fs_clock: wx.2,
        fs_clock_tick,
        smooth_seek_debounce: sc.7,
        resume_after_seek_idle: sc.8,
        play_toggle: args.play_ctx.clone(),
        dvd_bar: sc.9,
        hdr_title_mirror: sc.10,
        speed_sync: wx.3,
        speed_menu: args.w.speed_mbtn.clone(),
        speed_list: args.w.speed_list.clone(),
        speed_readout: wx.4,
    });
}
