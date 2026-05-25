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
include!("build_window/wire_handlers_before_mpv.rs");

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
    let dvd_bar = Rc::new(RefCell::new(None::<crate::dvd_vob_timeline::DvdBarState>));
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
    let fs_pause_stash = Rc::new(RefCell::new(None::<bool>));
    let fs_transition_busy = Rc::new(Cell::new(false));
    let fs_transition_settle = Rc::new(RefCell::new(None::<glib::SourceId>));
    let skip_max_to_fs = Rc::new(Cell::new(false));
    let last_unmax = Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H)));
    let win_aspect = Rc::new(Cell::new(None::<f64>));
    let aspect_resize_end_deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let aspect_resize_wired = Rc::new(Cell::new(false));
    let idle_inhib = Rc::new(RefCell::new(None::<crate::idle_inhibit::Held>));
    let mpv_teardown_after_draw = Rc::new(Cell::new(false));

    let h = wire_handlers_before_mpv(
        app, &w, player, &file_boot, &on_open_slot, &sub_pref, &video_pref, &bar_show,
        &nav_t, &motion_squelch, &fs_restore, &last_unmax, &skip_max_to_fs, &fs_transition_busy,
        &last_path, &playback_focus, &sibling_seof, &win_aspect, &dvd_bar,
    );
    let HandlersBeforeMpv {
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
        pending_recent_backfill,
        undo_remove_stack,
        undo_timer,
        do_commit,
        recent_visible,
        close_action_cell,
        trash_action_cell,
        warm_preload,
    } = h;

    let video_file_actions = wire_video_file_actions(VideoFileActionCtx {
        app: app.clone(), player: player.clone(), recent: w.recent_scrl.clone(),
        on_browse_back: Rc::clone(&on_browse_back), undo_timer: undo_timer.clone(),
        undo_remove_stack: undo_remove_stack.clone(), do_commit: do_commit.clone(),
        close_action_cell: Rc::clone(&close_action_cell), trash_action_cell: Rc::clone(&trash_action_cell),
        close_video_btn: w.close_video_btn.clone(),
    });

    wire_mpv_realize(MpvRealizeCtx {
        player: player.clone(), sub_pref: sub_pref.clone(), video_pref: Rc::clone(&video_pref),
        app: app.clone(), win: w.win.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), bar_show: bar_show.clone(), bottom: w.bottom.clone(),
        last_path: last_path.clone(),         on_video_chrome: Rc::clone(&on_video_chrome),
        on_file_loaded: Rc::clone(&on_file_loaded), file_boot: Rc::clone(&file_boot),
        win_aspect: Rc::clone(&win_aspect), reapply_60: reapply_60.clone(),
        pending_recent_backfill: Rc::clone(&pending_recent_backfill),
        close_video: video_file_actions.close_video,
        move_to_trash: video_file_actions.move_to_trash,
        mpv_teardown_after_draw: Rc::clone(&mpv_teardown_after_draw),
        hdr_title_mirror: w.hdr_title_mirror.clone(), playback_focus: Rc::clone(&playback_focus),
        close_video_btn: w.close_video_btn.clone(),
    });

    let vol_sync = Rc::new(Cell::new(false));
    let hdr_title_mirror = w.hdr_title_mirror.clone();
    let win_present = w.win.clone();
    stash_after_present_args(WindowAfterPresentArgs {
        app: app.clone(),
        w,
        player: Rc::clone(player),
        video_pref,
        sub_pref,
        seek_chapters,
        dvd_bar,
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
        warm_preload,
        continue_grid_cache,
    });

    crate::window_present::present_on_activation_display(&win_present);
}

include!("build_window/deferred_after_present.rs");

include!("build_window/popover_shows.rs");
