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
    win_aspect: &Rc<Cell<Option<f64>>>,
) -> HandlersBeforeMpv {
    #[cfg(target_os = "macos")]
    crate::macos_window::register_win_bar_show(&w.win, Rc::clone(bar_show), w.root.clone());
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
    crate::screen_blackout::wire_blackout_hooks(&w.blackout_sync, &w.blackout_menu);
    let (seek_sync, seek_grabbed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let smooth_seek_debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
    let resume_after_seek_idle = Rc::new(Cell::new(false));

    let undo_shell = w.undo_bar.shell.clone();
    let undo_label = w.undo_bar.label.clone();
    let undo_btn = w.undo_bar.undo.clone();
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
    attach_window_shell(&WindowInputShell {
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
    });
    let shell_layout = Rc::new(ShellLayoutCtx {
        win: w.win.clone(),
        root: w.root.clone(),
        header: w.header.clone(),
        video_handle: w.video_handle.clone(),
        gl: w.gl_area.clone(),
        bottom: w.bottom.clone(),
        #[cfg(target_os = "macos")]
        bottom_shell: w.bottom_shell.clone(),
        recent: w.recent_scrl.clone(),
        bar_show: Rc::clone(bar_show),
        player: Rc::clone(player),
        touch_chrome: RefCell::new(None),
    });
    register_shell_layout(Rc::clone(&shell_layout));
    #[cfg(target_os = "macos")]
    {
        wire_macos_recent_hide_refresh(&w.win, &w.gl_area, &w.recent_scrl, player);
        wire_macos_surface_compositing_refresh(&shell_layout);
    }

    let hdr_csd_baseline = Rc::new(Cell::new(None));
    wire_header_csd_baseline_snap(&hdr_csd_baseline, &w.header);

    let ch_hide = Rc::new(ChromeBarHide {
        nav: nav_t.clone(), vol: w.vol_menu.clone(), sub: w.sub_menu.clone(),
        speed: w.speed_mbtn.clone(), main: w.menu_btn.clone(), win: w.win.clone(),
        root: w.root.clone(),
        header: w.header.clone(), gl: w.gl_area.clone(), bar_show: bar_show.clone(),
        recent: w.recent_scrl.clone(),
        bottom: w.bottom.clone(), player: player.clone(), squelch: motion_squelch.clone(),
        seek_grabbed: seek_grabbed.clone(),
        hdr_csd_baseline: Rc::clone(&hdr_csd_baseline),
    });
    #[cfg(target_os = "macos")]
    {
        let chc = Rc::clone(&ch_hide);
        let chc_pop = Rc::clone(&chc);
        crate::macos_header_menu::register_checks(
            Rc::new(move || {
                chc.vol.is_active()
                    || chc.sub.is_active()
                    || chc.speed.is_active()
                    || chc.vol.popover().is_some_and(|p| p.is_visible())
                    || chc.sub.popover().is_some_and(|p| p.is_visible())
                    || chc.speed.popover().is_some_and(|p| p.is_visible())
                    || crate::macos_header_menu_overlay::overlay_visible()
            }),
            Rc::new(move || {
                chc_pop.vol.popover().is_some_and(|p| p.is_visible())
                    || chc_pop.sub.popover().is_some_and(|p| p.is_visible())
                    || chc_pop.speed.popover().is_some_and(|p| p.is_visible())
            }),
        );
    }
    let on_video_chrome: Rc<dyn Fn()> = {
        let (csp, root, gl, b, recent, bot, p, hdr, chh, win_ov) = (
            Rc::clone(&hdr_csd_baseline),
            w.root.clone(),
            w.gl_area.clone(),
            bar_show.clone(),
            w.recent_scrl.clone(),
            w.bottom.clone(),
            player.clone(),
            w.header.clone(),
            Rc::clone(&ch_hide),
            w.win.clone(),
        );
        Rc::new(move || {
            b.set(true);
            apply_chrome(ChromeApplyParts {
                hdr_csd_baseline: &csp,
                root: &root,
                header: &hdr,
                gl: &gl,
                bar_show: &b,
                recent: &recent,
                bottom: &bot,
                player: &p,
            });
            schedule_bars_autohide(Rc::clone(&chh));
            show_chrome_pointer(&win_ov, &gl);
        })
    };
    wire_shell_layout_chrome(Rc::clone(&on_video_chrome));
    wire_menu_chrome(
        Rc::clone(&ch_hide),
        &w.vol_menu,
        &w.sub_menu,
        &w.speed_mbtn,
        &w.menu_btn,
    );
    let play_ctx = PlayToggleCtx {
        app: app.clone(), player: player.clone(), video_pref: Rc::clone(video_pref),
        win: w.win.clone(),
        video_handle: w.video_handle.clone(),
        gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect: win_aspect.clone(), sub_menu: Some(w.sub_menu.clone()),
        play_pause: w.play_pause.clone(),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(playback_focus),
    };
    wire_play_toggles(&w.play_pause, play_ctx.clone());
    let browse_chrome = rc_on_browse_chrome(BrowseChromeRefs {
        hdr_csd: Rc::clone(&hdr_csd_baseline),
        nav_t: nav_t.clone(),
        win: w.win.clone(),
        root: w.root.clone(),
        gl: w.gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: w.recent_scrl.clone(),
        bottom: w.bottom.clone(),
        player: player.clone(),
        header: w.header.clone(),
    });
    let on_open = make_on_open_handler(OpenHandlerCtx {
        player: player.clone(), win: w.win.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), last_path: last_path.clone(), video_pref: Rc::clone(video_pref),
        on_start: on_video_chrome.clone(), on_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(win_aspect),
        sub_menu: w.sub_menu.clone(),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(playback_focus),
    });
    *on_open_slot.borrow_mut() = Some(on_open.clone());
    wire_window_drop_targets(&w.win, player, &w.sub_menu, &on_open);
    wire_sibling_navigation(SiblingNavCtx {
        btn_prev: w.sibling_nav.prev_btn.clone(),
        btn_next: w.sibling_nav.next_btn.clone(),
        win: w.win.clone(),
        gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(),
        player: player.clone(),
        last_path: last_path.clone(),
        video_pref: Rc::clone(video_pref),
        on_video_chrome: on_video_chrome.clone(),
        win_aspect: win_aspect.clone(),
        sibling_seof: sibling_seof.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(playback_focus),
    });

    let warm_preload = want_recent.then(|| {
        WarmPreloadCtx::new(
            player.clone(),
            Rc::clone(video_pref),
            w.recent_scrl.clone(),
            w.gl_area.clone(),
            last_path.clone(),
        )
    });
    let warm_hover = warm_preload
        .as_ref()
        .map(|ctx| warm_hover_hooks(Rc::clone(ctx)));
    let continue_grid_cache = Rc::new(RefCell::new(std::collections::HashMap::new()));
    let recent_wiring = wire_recent_undo(RecentUndoCtx {
        player: player.clone(), recent: w.recent_scrl.clone(), flow: w.flow_recent.clone(),
        undo_shell: undo_shell.clone(), undo_label: undo_label.clone(), undo_btn: undo_btn.clone(),
        undo_close: w.undo_bar.close.clone(), on_open: on_open.clone(), want_recent,
        warm_hover: warm_hover.clone(),
        continue_grid_cache: Rc::clone(&continue_grid_cache),
    });
    // `is_visible()` is false until the window is mapped; use `want_recent` so transport
    // and warm-preload see the continue strip on empty launch before `present`.
    let recent_visible = Rc::new(Cell::new(want_recent));
    {
        let rv = Rc::clone(&recent_visible);
        w.recent_scrl
            .connect_notify_local(Some("visible"), move |r, _| rv.set(r.is_visible()));
    }
    let on_browse_back = make_browse_back(
        BackToBrowseCtx {
            player: player.clone(),
            close_video_btn: w.close_video_btn.clone(),
            close_action_cell: Rc::clone(&close_act_for_sync),
            on_open: on_open.clone(),
            on_remove: recent_wiring.on_remove.clone(),
            on_trash: recent_wiring.on_trash.clone(),
            recent_backfill: recent_wiring.recent_backfill.clone(),
            last_path: last_path.clone(),
            sibling_seof: sibling_seof.clone(),
            sibling_nav: w.sibling_nav.clone(),
            win_aspect: win_aspect.clone(),
            on_browse: browse_chrome.clone(),
            undo_shell: undo_shell.clone(),
            undo_label: undo_label.clone(),
            undo_btn: undo_btn.clone(),
            undo_timer: recent_wiring.undo_timer.clone(),
            undo_remove_stack: recent_wiring.undo_remove_stack.clone(),
            recent_visible: Rc::clone(&recent_visible),
            playback_focus: Rc::clone(playback_focus),
            browse_has_strip: want_recent,
            hdr_title_mirror: w.hdr_title_mirror.clone(),
            continue_grid_cache: Rc::clone(&continue_grid_cache),
        },
        w.win.clone(), w.gl_area.clone(), w.recent_scrl.clone(), w.flow_recent.clone(),
    );

    HandlersBeforeMpv {
        continue_grid_cache,
        seek_sync,
        seek_grabbed,
        smooth_seek_debounce,
        resume_after_seek_idle,
        hdr_csd_baseline,
        ch_hide,
        on_video_chrome,
        on_file_loaded,
        play_ctx,
        on_browse_back,
        pending_recent_backfill: recent_wiring.pending_recent_backfill,
        undo_remove_stack: recent_wiring.undo_remove_stack,
        undo_timer: recent_wiring.undo_timer,
        do_commit: recent_wiring.do_commit,
        recent_visible,
        close_action_cell: close_act_for_sync,
        trash_action_cell: trash_act_for_sync,
        warm_preload,
    }
}
