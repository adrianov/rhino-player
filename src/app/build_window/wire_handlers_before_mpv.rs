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

fn wire_handlers_before_mpv(
    app: &adw::Application,
    w: &WindowWidgets,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: &Rc<RefCell<Option<RcPathFn>>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    bar_show: &Rc<Cell<bool>>,
    nav_t: &Rc<RefCell<Option<glib::SourceId>>>,
    motion_squelch: &Rc<Cell<Option<Instant>>>,
    fs_restore: &Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &Rc<Cell<bool>>,
    fs_transition_busy: &Rc<Cell<bool>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    playback_focus: &Rc<Cell<bool>>,
    sibling_seof: &Rc<SiblingEofState>,
    win_aspect: &Rc<WinAspectCell>,
    dvd_bar: &Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
) -> HandlersBeforeMpv {
    #[cfg(target_os = "macos")]
    wire_macos_header_menu_cluster(
        &w.root,
        &w.header,
        &w.outer_ovl,
        &w.win,
        &[
            (
                w.speed_mbtn.clone(),
                w.speed_mbtn.popover().expect("speed popover"),
                "speed",
            ),
            (w.sub_menu.clone(), w.sub_pop.clone(), "subtitles"),
            (w.vol_menu.clone(), w.vol_pop.clone(), "audio"),
        ],
    );
    #[cfg(not(target_os = "macos"))]
    header_menubtns_switch(&[
        w.speed_mbtn.clone(), w.sub_menu.clone(), w.vol_menu.clone(), w.menu_btn.clone(),
    ]);

    wire_popover_shows(player, w, sub_pref);
    crate::screen_blackout::wire_blackout_hooks(&w.blackout_sync);
    let (seek_sync, seek_grabbed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let smooth_seek_debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
    let resume_after_seek_idle = Rc::new(Cell::new(false));

    let notice_ctrl = crate::recent_view::NoticeToastCtrl::new(w.notice_toast.clone());
    let browse_prep = ContinueBrowsePrep::start(
        notice_ctrl,
        w.recent_scrl.clone(),
        Rc::clone(playback_focus),
    );
    let on_open_fail = Rc::clone(&browse_prep.on_open_fail);
    let close_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));
    let trash_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));

    let on_file_loaded = make_file_loaded_handler(FileLoadedCtx {
        player: player.clone(), last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(), sibling_nav: w.sibling_nav.clone(),
        sub_pref: sub_pref.clone(), gl: w.gl_area.clone(), bar_show: bar_show.clone(),
        recent: w.recent_scrl.clone(), bottom: w.bottom.clone(), sub_menu: w.sub_menu.clone(),
        close_action_cell: Rc::clone(&close_act_for_sync),
        trash_action_cell: Rc::clone(&trash_act_for_sync),
        speed_sync: w.speed_sync.clone(),
        speed_menu: w.speed_mbtn.clone(),
        speed_list: w.speed_list.clone(),
        speed_readout: w.speed_readout.clone(),
        video_pref: Rc::clone(video_pref), app: app.clone(), close_video_btn: w.close_video_btn.clone(),
    });
    wire_sub_style_controls(SubStyleCtx {
        player: player.clone(), sub_pref: sub_pref.clone(), gl: w.gl_area.clone(),
        bar_show: bar_show.clone(), recent: w.recent_scrl.clone(), bottom: w.bottom.clone(),
        sub_scale_adj: w.sub_scale_adj.clone(), sub_color_btn: w.sub_color_btn.clone(),
    });

    let fs_toggle = FullscreenToggleRefs {
        fs_restore: Rc::clone(fs_restore),
        last_unmax: Rc::clone(last_unmax),
        skip_max_to_fs: Rc::clone(skip_max_to_fs),
        fs_transition_busy: Rc::clone(fs_transition_busy),
    };

    wire_gl_double_click_fullscreen(&w.gl_area, &w.win, &fs_toggle, &w.recent_scrl);
    wire_header_fullscreen_toggle(&w.header, &w.win, &fs_toggle, &w.recent_scrl);
    wire_recent_spacer_fullscreen(
        w.recent_spacers.clone(), &w.win, &fs_toggle, &w.recent_scrl,
    );

    let want_recent = file_boot.borrow().is_none();
    w.recent_scrl.set_visible(want_recent);
    let video_chrome = VideoChrome::attach(VideoChromeParts {
        win: &w.win,
        root: &w.root,
        header: &w.header,
        outer_ovl: &w.outer_ovl,
        video_handle: &w.video_handle,
        gl: &w.gl_area,
        recent: &w.recent_scrl,
        bottom: &w.bottom,
        #[cfg(target_os = "macos")]
        bottom_shell: &w.bottom_shell,
        player,
        bar_show,
        nav_t,
        motion_squelch,
        seek_grabbed: &seek_grabbed,
        vol_menu: &w.vol_menu,
        sub_menu: &w.sub_menu,
        speed_mbtn: &w.speed_mbtn,
        menu_btn: &w.menu_btn,
    });
    let hdr_csd_baseline = Rc::clone(&video_chrome.hdr_csd_baseline);
    let ch_hide = Rc::clone(&video_chrome.ch_hide);
    let on_video_chrome = Rc::clone(&video_chrome.on_show);
    let media_open = MediaOpenWire::attach(MediaOpenParts {
        app,
        w,
        player,
        video_pref,
        last_path,
        on_video_chrome: Rc::clone(&on_video_chrome),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect,
        playback_focus,
        sibling_seof,
        on_open_fail: Rc::clone(&on_open_fail),
        on_open_slot,
    });
    let browse = browse_prep.finish(ContinueBrowseFinish {
        want_recent,
        player,
        video_pref,
        w,
        last_path,
        on_open: media_open.on_open.clone(),
        sibling_seof,
        win_aspect,
        playback_focus,
        close_action_cell: Rc::clone(&close_act_for_sync),
        dvd_bar,
        hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
        nav_t,
        bar_show,
    });

    HandlersBeforeMpv {
        continue_grid_cache: browse.continue_grid_cache,
        seek_sync,
        seek_grabbed,
        smooth_seek_debounce,
        resume_after_seek_idle,
        hdr_csd_baseline,
        ch_hide,
        on_video_chrome,
        on_file_loaded,
        play_ctx: media_open.play_ctx,
        on_browse_back: browse.on_browse_back,
        pending_recent_backfill: browse.pending_recent_backfill,
        undo_remove_stack: browse.undo_remove_stack,
        undo_timer: browse.undo_timer,
        do_commit: browse.do_commit,
        recent_visible: browse.recent_visible,
        close_action_cell: close_act_for_sync,
        trash_action_cell: trash_act_for_sync,
        warm_preload: browse.warm_preload,
        on_open_fail: browse.on_open_fail,
    }
}
