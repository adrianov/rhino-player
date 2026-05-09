include!("build_window/app_menus.rs");
include!("build_window/linux_main_menu_button.rs");
include!("build_window/aspect_resize.rs");
include!("build_window/header_popovers.rs");
include!("build_window/sibling_nav_buttons.rs");
include!("build_window/wire_mpris_linux.rs");
include!("build_window/speed_menu.rs");
include!("build_window/smooth_video_toolbar.rs");
include!("build_window/volume_wiring.rs");
include!("build_window/widgets.rs");
include!("build_window/wire_drag_drop.rs");
include!("build_window/header_fullscreen_toggle.rs");
include!("build_window/browse_chrome_hover.rs");

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: Rc<RefCell<Option<RcPathFn>>>,
) {
    let sub_pref = Rc::new(RefCell::new(db::load_sub()));
    let video_pref = Rc::new(RefCell::new(db::load_video()));
    let reapply_60 = VideoReapply60 { vp: Rc::clone(&video_pref), app: app.clone() };
    let exit_after_current = Rc::new(Cell::new(false));
    let w = build_widgets(
        app, player, &video_pref, &sub_pref, Rc::clone(&exit_after_current),
    );

    let seek_chapters = Rc::new(RefCell::new(Vec::<(f64, String)>::new()));
    let bar_show = Rc::new(Cell::new(true));
    let nav_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let cur_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let ptr_in_gl = Rc::new(Cell::new(false));
    let motion_squelch = Rc::new(Cell::new(None::<Instant>));
    let last_cap_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_gl_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_path = Rc::new(RefCell::new(None::<PathBuf>));
    let playback_focus = Rc::new(Cell::new(false));
    let seek_bar_on = Rc::new(Cell::new(db::load_seek_bar_preview()));
    let sibling_seof = Rc::new(SiblingEofState {
        done: Cell::new(false),
        nav_key: RefCell::new(None),
        nav_can_prev: Cell::new(false),
        nav_can_next: Cell::new(false),
    });
    let fs_restore = Rc::new(RefCell::new(None::<(i32, i32)>));
    let fs_transition_busy = Rc::new(Cell::new(false));
    let fs_transition_settle = Rc::new(RefCell::new(None::<glib::SourceId>));
    let skip_max_to_fs = Rc::new(Cell::new(false));
    let last_unmax = Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H)));
    let win_aspect = Rc::new(Cell::new(None::<f64>));
    let aspect_resize_end_deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let aspect_resize_wired = Rc::new(Cell::new(false));
    let idle_inhib = Rc::new(RefCell::new(None::<u32>));
    let mpv_teardown_after_draw = Rc::new(Cell::new(false));

    #[cfg(target_os = "macos")]
    header_menubtns_switch(&[w.speed_mbtn.clone(), w.sub_menu.clone(), w.vol_menu.clone()]);
    #[cfg(not(target_os = "macos"))]
    header_menubtns_switch(&[
        w.speed_mbtn.clone(), w.sub_menu.clone(), w.vol_menu.clone(), w.menu_btn.clone(),
    ]);

    wire_popover_shows(player, &w, &sub_pref);
    let (seek_sync, seek_grabbed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let smooth_seek_debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
    let resume_after_seek_idle = Rc::new(Cell::new(false));
    let seek_preview = seek_bar_preview::connect(
        &w.seek, &w.seek_adj, Rc::clone(player), Rc::clone(&last_path),
        Rc::clone(&seek_bar_on), Rc::clone(&seek_chapters),
        seek_bar_preview::SeekPreviewCtx { ovl: w.outer_ovl.clone(), bottom: w.bottom.clone() },
    );
    w.outer_ovl.add_overlay(&seek_preview.container);

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
        video_pref: Rc::clone(&video_pref), app: app.clone(), close_video_btn: w.close_video_btn.clone(),
        playback_focus: Rc::clone(&playback_focus),
    });
    wire_sub_style_controls(SubStyleCtx {
        player: player.clone(), sub_pref: sub_pref.clone(), gl: w.gl_area.clone(),
        bar_show: bar_show.clone(), recent: w.recent_scrl.clone(), bottom: w.bottom.clone(),
        sub_scale_adj: w.sub_scale_adj.clone(), sub_color_btn: w.sub_color_btn.clone(),
    });

    let fs_toggle = FullscreenToggleRefs {
        fs_restore: Rc::clone(&fs_restore),
        last_unmax: Rc::clone(&last_unmax),
        skip_max_to_fs: Rc::clone(&skip_max_to_fs),
        fs_transition_busy: Rc::clone(&fs_transition_busy),
    };

    wire_gl_double_click_fullscreen(
        &w.gl_area,
        &w.win,
        &fs_toggle,
        &w.recent_scrl,
    );

    wire_header_fullscreen_toggle(
        &w.header,
        &w.win,
        &fs_toggle,
        &w.recent_scrl,
    );

    wire_recent_spacer_fullscreen(
        w.recent_spacers, &w.win, &fs_toggle, &w.recent_scrl,
    );

    let want_recent = file_boot.borrow().is_none();
    w.recent_scrl.set_visible(want_recent);

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
    wire_menu_chrome(Rc::clone(&ch_hide), &w.vol_menu, &w.sub_menu, &w.speed_mbtn, &w.menu_btn);
    let play_ctx = PlayToggleCtx {
        app: app.clone(), player: player.clone(), video_pref: Rc::clone(&video_pref),
        win: w.win.clone(),
        video_handle: w.video_handle.clone(),
        gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect: win_aspect.clone(), sub_menu: Some(w.sub_menu.clone()),
        play_pause: w.play_pause.clone(),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
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
        recent: w.recent_scrl.clone(), last_path: last_path.clone(),
        on_start: on_video_chrome.clone(), on_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(&win_aspect),
        sub_menu: w.sub_menu.clone(),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&playback_focus),
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
        on_video_chrome: on_video_chrome.clone(),
        win_aspect: win_aspect.clone(),
        sibling_seof: sibling_seof.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&playback_focus),
    });

    let recent_wiring = wire_recent_undo(RecentUndoCtx {
        player: player.clone(), recent: w.recent_scrl.clone(), flow: w.flow_recent.clone(),
        undo_shell: undo_shell.clone(), undo_label: undo_label.clone(), undo_btn: undo_btn.clone(),
        undo_close: w.undo_bar.close.clone(), on_open: on_open.clone(), want_recent,
    });
    let recent_backfill = recent_wiring.recent_backfill;
    let pending_recent_backfill = recent_wiring.pending_recent_backfill;
    let undo_remove_stack = recent_wiring.undo_remove_stack;
    let undo_timer = recent_wiring.undo_timer;
    let do_commit = recent_wiring.do_commit;
    let on_remove = recent_wiring.on_remove;
    let on_trash = recent_wiring.on_trash;
    let recent_visible = Rc::new(Cell::new(w.recent_scrl.is_visible()));
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
            on_remove: on_remove.clone(), on_trash: on_trash.clone(),
            recent_backfill: recent_backfill.clone(), last_path: last_path.clone(),
            sibling_seof: sibling_seof.clone(), sibling_nav: w.sibling_nav.clone(),
            win_aspect: win_aspect.clone(), on_browse: browse_chrome.clone(),
            undo_shell: undo_shell.clone(), undo_label: undo_label.clone(),
            undo_btn: undo_btn.clone(), undo_timer: undo_timer.clone(),
            undo_remove_stack: undo_remove_stack.clone(),
            recent_visible: Rc::clone(&recent_visible),
            playback_focus: Rc::clone(&playback_focus),
            browse_has_strip: want_recent,
            hdr_title_mirror: w.hdr_title_mirror.clone(),
        },
        w.win.clone(), w.gl_area.clone(), w.recent_scrl.clone(), w.flow_recent.clone(),
    );

    include!("build_window/install_window_input.rs");

    let video_file_actions = wire_video_file_actions(VideoFileActionCtx {
        app: app.clone(), player: player.clone(), recent: w.recent_scrl.clone(),
        on_browse_back: on_browse_back.clone(), undo_timer: undo_timer.clone(),
        undo_remove_stack: undo_remove_stack.clone(), do_commit: do_commit.clone(),
        close_action_cell: Rc::clone(&close_act_for_sync), trash_action_cell: Rc::clone(&trash_act_for_sync),
        playback_focus: Rc::clone(&playback_focus), close_video_btn: w.close_video_btn.clone(),
    });

    wire_mpv_realize(MpvRealizeCtx {
        player: player.clone(), sub_pref: sub_pref.clone(), video_pref: Rc::clone(&video_pref),
        app: app.clone(), win: w.win.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), bar_show: bar_show.clone(), bottom: w.bottom.clone(),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded), file_boot: Rc::clone(&file_boot),
        win_aspect: Rc::clone(&win_aspect), reapply_60: reapply_60.clone(),
        pending_recent_backfill: pending_recent_backfill.clone(),
        close_video: video_file_actions.close_video,
        move_to_trash: video_file_actions.move_to_trash,
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        hdr_title_mirror: w.hdr_title_mirror.clone(), playback_focus: Rc::clone(&playback_focus),
        close_video_btn: w.close_video_btn.clone(),
    });

    include!("build_window/post_seek_mpris.rs");

    let vol_sync = Rc::new(Cell::new(false));
    wire_volume_controls(VolumeCtx {
        player: player.clone(), recent: w.recent_scrl.clone(), gl: w.gl_area.clone(),
        vol_header_img: w.vol_header_img.clone(), vol_readout: w.vol_readout.clone(),
        vol_adj: w.vol_adj.clone(), vol_mute_btn: w.vol_mute_btn.clone(), vol_sync: vol_sync.clone(),
    });

    include!("build_window/aspect_transport_final.rs");
}

include!("build_window/popover_shows.rs");
