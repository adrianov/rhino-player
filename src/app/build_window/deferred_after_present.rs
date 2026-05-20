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
fn wire_window_after_present(args: WindowAfterPresentArgs) {
    let WindowAfterPresentArgs {
        app,
        w,
        player,
        video_pref,
        sub_pref,
        seek_chapters,
        seek_bar_on,
        last_path,
        bar_show,
        nav_t,
        cur_t,
        ptr_in_gl,
        motion_squelch,
        last_cap_xy,
        last_gl_xy,
        fs_restore,
        fs_pause_stash,
        fs_transition_busy,
        fs_transition_settle,
        skip_max_to_fs,
        last_unmax,
        ch_hide,
        hdr_csd_baseline,
        on_browse_back,
        on_video_chrome,
        on_file_loaded,
        win_aspect,
        sibling_seof,
        playback_focus,
        play_ctx,
        seek_sync,
        seek_grabbed,
        smooth_seek_debounce,
        resume_after_seek_idle,
        idle_inhib,
        exit_after_current,
        mpv_teardown_after_draw,
        reapply_60,
        recent_visible,
        hdr_title_mirror,
        vol_sync,
        aspect_resize_end_deb,
        aspect_resize_wired,
        file_boot,
    } = args;

    // Same as continue-strip launch (`file_boot` none); do not use `recent_visible.get()`
    // here — it may still be false before the window is mapped.
    let want_warm_preload =
        file_boot.borrow().is_none() && last_path.borrow().is_none();

    let preview_hover_t = if seek_bar_on.get() {
        let seek_preview = seek_bar_preview::connect(
            &w.seek,
            &w.seek_adj,
            Rc::clone(&player),
            Rc::clone(&last_path),
            Rc::clone(&seek_bar_on),
            Rc::clone(&seek_chapters),
            seek_bar_preview::SeekPreviewCtx {
                ovl: w.outer_ovl.clone(),
                bottom: w.bottom.clone(),
            },
        );
        w.outer_ovl.add_overlay(&seek_preview.container);
        Rc::clone(&seek_preview.hover_t)
    } else {
        Rc::new(Cell::new(0.0))
    };

    let fs_clock_tick = Rc::new(RefCell::new(None::<glib::SourceId>));
    wire_window_input(WindowInputCtx {
        shell: WindowInputShell {
            win: w.win.clone(),
            root: w.root.clone(),
            header: w.header.clone(),
            outer_ovl: w.outer_ovl.clone(),
            video_handle: w.video_handle.clone(),
            bottom: w.bottom.clone(),
            gl: w.gl_area.clone(),
            recent: w.recent_scrl.clone(),
        },
        app: app.clone(),
        player: player.clone(),
        video_pref: Rc::clone(&video_pref),
        bar_show: bar_show.clone(),
        nav_t: nav_t.clone(),
        cur_t: cur_t.clone(),
        ptr_in_gl: ptr_in_gl.clone(),
        motion_squelch: motion_squelch.clone(),
        last_cap_xy: last_cap_xy.clone(),
        last_gl_xy: last_gl_xy.clone(),
        fs_restore: fs_restore.clone(),
        fs_pause_stash: fs_pause_stash.clone(),
        fs_transition_busy: fs_transition_busy.clone(),
        fs_transition_settle: fs_transition_settle.clone(),
        skip_max_to_fs: skip_max_to_fs.clone(),
        last_unmax: last_unmax.clone(),
        ch_hide: Rc::clone(&ch_hide),
        hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
        on_browse_back: on_browse_back.clone(),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        last_path: last_path.clone(),
        win_aspect: win_aspect.clone(),
        sibling_seof: sibling_seof.clone(),
        playback_focus: Rc::clone(&playback_focus),
        play_pause: w.play_pause.clone(),
        seek: w.seek.clone(),
        seek_sync: seek_sync.clone(),
        time_left: w.time_left.clone(),
        fs_clock: w.fs_clock.clone(),
        fs_clock_tick,
        smooth_seek_debounce: smooth_seek_debounce.clone(),
        resume_after_seek_idle: resume_after_seek_idle.clone(),
        play_toggle: play_ctx.clone(),
        hdr_title_mirror: hdr_title_mirror.clone(),
        speed_sync: w.speed_sync.clone(),
        speed_list: w.speed_list.clone(),
        speed_readout: w.speed_readout.clone(),
    });

    #[cfg(target_os = "macos")]
    {
        let nav = SiblingNavCtx {
            btn_prev: w.sibling_nav.prev_btn.clone(),
            btn_next: w.sibling_nav.next_btn.clone(),
            win: w.win.clone(),
            gl: w.gl_area.clone(),
            recent: w.recent_scrl.clone(),
            player: player.clone(),
            last_path: last_path.clone(),
            video_pref: Rc::clone(&video_pref),
            on_video_chrome: on_video_chrome.clone(),
            win_aspect: win_aspect.clone(),
            sibling_seof: sibling_seof.clone(),
            on_file_loaded: Rc::clone(&on_file_loaded),
            hdr_title_mirror: hdr_title_mirror.clone(),
            playback_focus: Rc::clone(&playback_focus),
        };
        wire_macos_now_playing_remote(play_ctx.clone(), nav);
    }

    wire_seek_control(&w.seek, SeekControlDeps {
        player: player.clone(),
        gl: w.gl_area.clone(),
        seek_sync: seek_sync.clone(),
        seek_grabbed: seek_grabbed.clone(),
        time_left: w.time_left.clone(),
        preview_hover_t: preview_hover_t.clone(),
        smooth_seek_debounce: smooth_seek_debounce.clone(),
        resume_after_seek_idle: resume_after_seek_idle.clone(),
        play_toggle: play_ctx.clone(),
    });

    #[cfg(target_os = "linux")]
    wire_mpris_linux_after_seek(MprisLinuxWireCtx {
        app: &app,
        win: w.win.clone(),
        gl_area: w.gl_area.clone(),
        recent_scrl: w.recent_scrl.clone(),
        player: &player,
        play_ctx: &play_ctx,
        last_path: &last_path,
        win_aspect: &win_aspect,
        sibling_seof: &sibling_seof,
        video_pref: Rc::clone(&play_ctx.video_pref),
        smooth_seek_debounce: smooth_seek_debounce.clone(),
        resume_after_seek_idle: resume_after_seek_idle.clone(),
        on_file_loaded: &on_file_loaded,
        on_video_chrome: &on_video_chrome,
        hdr_title_mirror: hdr_title_mirror.clone(),
        playback_focus: &playback_focus,
    });

    wire_volume_controls(VolumeCtx {
        player: player.clone(),
        recent: w.recent_scrl.clone(),
        gl: w.gl_area.clone(),
        vol_header_img: w.vol_header_img.clone(),
        vol_readout: w.vol_readout.clone(),
        vol_adj: w.vol_adj.clone(),
        vol_mute_btn: w.vol_mute_btn.clone(),
        vol_sync: vol_sync.clone(),
    });

    wire_aspect_resize_on_map(
        &w.win, &w.recent_scrl, &win_aspect, &aspect_resize_end_deb, &aspect_resize_wired,
    );

    wire_transport_events(TransportSetup {
        app: app.clone(),
        player: player.clone(),
        video_pref: Rc::clone(&video_pref),
        sub_pref: sub_pref.clone(),
        win: w.win.clone(),
        gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(),
        recent_visible: Rc::clone(&recent_visible),
        playback_focus: Rc::clone(&playback_focus),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        sibling_nav: w.sibling_nav.clone(),
        exit_after_current: exit_after_current.clone(),
        win_aspect: win_aspect.clone(),
        idle_inhib: Rc::clone(&idle_inhib),
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
        bar_show: bar_show.clone(),
        hdr_title_mirror: hdr_title_mirror.clone(),
        seek_chapters: Rc::clone(&seek_chapters),
        blackout: Rc::clone(&w.blackout_sync),
        widgets: TransportWidgets {
            play_pause: w.play_pause.clone(),
            seek: w.seek.clone(),
            seek_adj: w.seek_adj.clone(),
            seek_sync: seek_sync.clone(),
            seek_grabbed: seek_grabbed.clone(),
            time_left: w.time_left.clone(),
            time_right: w.time_right.clone(),
            speed_menu: w.speed_mbtn.clone(),
            speed_readout: w.speed_readout.clone(),
            vol_menu: w.vol_menu.clone(),
            vol_header_img: w.vol_header_img.clone(),
            vol_readout: w.vol_readout.clone(),
            vol_adj: w.vol_adj.clone(),
            vol_mute: w.vol_mute_btn.clone(),
            vol_sync: vol_sync.clone(),
            sub_readout: w.sub_readout.clone(),
            smooth_toolbar_status: w.smooth_status.clone(),
        },
    });

    wire_final_actions(FinalActionCtx {
        app: app.clone(),
        win: w.win.clone(),
        fs_restore: Rc::clone(&fs_restore),
        fs_transition_busy: Rc::clone(&fs_transition_busy),
        last_unmax: Rc::clone(&last_unmax),
        skip_max_to_fs: Rc::clone(&skip_max_to_fs),
        root: w.root.clone(),
        header: w.header.clone(),
        gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(),
        bottom: w.bottom.clone(),
        player: player.clone(),
        sub_pref: sub_pref.clone(),
        video_pref: Rc::clone(&video_pref),
        playback_focus: Rc::clone(&playback_focus),
        #[cfg(target_os = "macos")]
        main_menu: w.main_menu.clone(),
        pref_menu: w.pref_menu.clone(),
        seek_bar_on: Rc::clone(&seek_bar_on),
        last_path: last_path.clone(),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(&win_aspect),
        bar_show: bar_show.clone(),
        idle_inhib: Rc::clone(&idle_inhib),
        exit_after_current: exit_after_current.clone(),
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
        hdr_title_mirror,
        smooth_toolbar_status: w.smooth_status.clone(),
    });

    if want_warm_preload {
        let player = Rc::clone(&player);
        let video_pref = Rc::clone(&video_pref);
        let recent = w.recent_scrl.clone();
        let last_path = Rc::clone(&last_path);
        let reapply_60 = reapply_60.clone();
        // Let the continue strip paint before `loadfile` (macOS beach-ball if we block the same
        // idle turn as transport wiring).
        let ctx = Rc::new(WarmPreloadCtx {
            gate: Rc::new(WarmPreloadGate {
                inflight: Cell::new(false),
                queued: RefCell::new(None),
            }),
            player,
            video_pref,
            recent,
            last_path,
        });
        let _ = glib::timeout_add_local(WARM_PRELOAD_DELAY, move || {
            run_continue_warm_preload(&ctx, &reapply_60, false);
            glib::ControlFlow::Break
        });
    }
}

struct WindowAfterPresentArgs {
    app: adw::Application,
    w: WindowWidgets,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
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
    win_aspect: Rc<Cell<Option<f64>>>,
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
}

