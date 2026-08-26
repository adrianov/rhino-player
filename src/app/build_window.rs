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
include!("build_window/video_chrome.rs");
include!("build_window/media_open_wire.rs");
include!("build_window/continue_browse.rs");
include!("build_window/wire_handlers_before_mpv.rs");

include!("build_window/state.rs");

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: Rc<RefCell<Option<RcPathFn>>>,
) {
    let bw = make_bw_state(app);
    let w = build_widgets(
        app,
        player,
        &bw.prefs.video_pref,
        &bw.prefs.sub_pref,
        Rc::clone(&bw.prefs.exit_after_current),
    );
    let h = wire_handlers_before_mpv(BeforeMpvRefs {
        app,
        w: &w,
        player,
        file_boot: &file_boot,
        on_open_slot: &on_open_slot,
        prefs: &bw.prefs,
        tl: &bw.tl,
        chrome: &bw.chrome,
    });
    let video_file_actions = wire_recent_undo_actions(app, &w, &h, player);
    wire_startup_realize(app, &w, &bw, &h, player, &file_boot, video_file_actions);
    stash_and_present(app, w, bw, h, player, file_boot);
}

/// Wires the recent-grid undo / close / trash actions shared by the header and cards.
fn wire_recent_undo_actions(
    app: &adw::Application,
    w: &WindowWidgets,
    h: &HandlersBeforeMpv,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) -> VideoFileActions {
    wire_video_file_actions(VideoFileActionCtx {
        app: app.clone(),
        player: player.clone(),
        recent: w.recent_scrl.clone(),
        on_browse_back: Rc::clone(&h.on_browse_back),
        undo_timer: h.undo_timer.clone(),
        undo_remove_stack: h.undo_remove_stack.clone(),
        do_commit: h.do_commit.clone(),
        close_action_cell: Rc::clone(&h.close_action_cell),
        trash_action_cell: Rc::clone(&h.trash_action_cell),
        close_video_btn: w.close_video_btn.clone(),
    })
}

/// Creates the mpv render bundle on `GLArea` realize (startup path) and wires frame drawing.
fn wire_startup_realize(
    app: &adw::Application,
    w: &WindowWidgets,
    bw: &BwState,
    h: &HandlersBeforeMpv,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &Rc<RefCell<Option<PathBuf>>>,
    video_file_actions: VideoFileActions,
) {
    wire_mpv_realize(MpvRealizeCtx {
        shell: RealizeShell {
            player: player.clone(),
            app: app.clone(),
            win: w.win.clone(),
            gl: w.gl_area.clone(),
            recent: w.recent_scrl.clone(),
            bottom: w.bottom.clone(),
            close_video_btn: w.close_video_btn.clone(),
            hdr_title_mirror: w.hdr_title_mirror.clone(),
        },
        st: realize_state(bw, h, file_boot, video_file_actions),
    });
}

/// Clones the shared state, handler callbacks, and teardown actions into the realize context.
fn realize_state(
    bw: &BwState,
    h: &HandlersBeforeMpv,
    file_boot: &Rc<RefCell<Option<PathBuf>>>,
    video_file_actions: VideoFileActions,
) -> RealizeState {
    RealizeState {
        sub_pref: bw.prefs.sub_pref.clone(),
        video_pref: Rc::clone(&bw.prefs.video_pref),
        bar_show: bw.tl.bar_show.clone(),
        last_path: bw.tl.last_path.clone(),
        on_video_chrome: Rc::clone(&h.on_video_chrome),
        on_file_loaded: Rc::clone(&h.on_file_loaded),
        file_boot: Rc::clone(file_boot),
        win_aspect: bw.chrome.win_aspect.clone(),
        pending_recent_backfill: Rc::clone(&h.pending_recent_backfill),
        close_video: video_file_actions.close_video,
        move_to_trash: video_file_actions.move_to_trash,
        mpv_teardown_after_draw: Rc::clone(&bw.chrome.mpv_teardown_after_draw),
        playback_focus: Rc::clone(&bw.tl.playback_focus),
        on_open_fail: Rc::clone(&h.on_open_fail),
    }
}

fn stash_and_present(
    app: &adw::Application,
    w: WindowWidgets,
    bw: BwState,
    h: HandlersBeforeMpv,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
) {
    let win_present = w.win.clone();
    stash_after_present_args(WindowAfterPresentArgs {
        app: app.clone(),
        hdr_title_mirror: w.hdr_title_mirror.clone(),
        w,
        player: Rc::clone(player),
        ch_hide: h.ch_hide,
        hdr_csd_baseline: h.hdr_csd_baseline,
        on_browse_back: h.on_browse_back,
        on_video_chrome: h.on_video_chrome,
        on_file_loaded: h.on_file_loaded,
        play_ctx: h.play_ctx,
        seek_sync: h.seek_sync,
        seek_grabbed: h.seek_grabbed,
        smooth_seek_debounce: h.smooth_seek_debounce,
        resume_after_seek_idle: h.resume_after_seek_idle,
        recent_visible: h.recent_visible,
        warm_preload: h.warm_preload,
        continue_grid_cache: h.continue_grid_cache,
        on_open_fail: h.on_open_fail,
        vol_sync: Rc::new(Cell::new(false)),
        video_pref: bw.prefs.video_pref,
        sub_pref: bw.prefs.sub_pref,
        seek_chapters: bw.tl.seek_chapters,
        dvd_bar: bw.tl.dvd_bar,
        seek_bar_on: bw.tl.seek_bar_on,
        last_path: bw.tl.last_path,
        bar_show: bw.tl.bar_show,
        nav_t: bw.tl.nav_t,
        cur_t: bw.tl.cur_t,
        ptr_in_gl: bw.tl.ptr_in_gl,
        motion_squelch: bw.tl.motion_squelch,
        last_cap_xy: bw.tl.last_cap_xy,
        last_gl_xy: bw.tl.last_gl_xy,
        fs_restore: bw.chrome.fs_restore,
        fs_pause_stash: bw.chrome.fs_pause_stash,
        fs_transition_busy: bw.chrome.fs_transition_busy,
        fs_transition_settle: bw.chrome.fs_transition_settle,
        skip_max_to_fs: bw.chrome.skip_max_to_fs,
        last_unmax: bw.chrome.last_unmax,
        win_aspect: bw.chrome.win_aspect,
        sibling_seof: bw.tl.sibling_seof,
        playback_focus: bw.tl.playback_focus,
        idle_inhib: bw.chrome.idle_inhib,
        exit_after_current: bw.prefs.exit_after_current,
        mpv_teardown_after_draw: bw.chrome.mpv_teardown_after_draw,
        reapply_60: bw.prefs.reapply_60,
        aspect_resize_end_deb: bw.chrome.aspect_resize_end_deb,
        aspect_resize_wired: bw.chrome.aspect_resize_wired,
        file_boot,
    });
    crate::window_present::present_on_activation_display(&win_present);
}

include!("build_window/deferred_after_present.rs");

include!("build_window/popover_shows.rs");
