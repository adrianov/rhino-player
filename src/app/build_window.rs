include!("build_window/app_menus.rs");
include!("build_window/aspect_resize.rs");
include!("build_window/header_popovers.rs");
include!("build_window/sibling_nav_buttons.rs");
include!("build_window/speed_menu.rs");
include!("build_window/volume_wiring.rs");
include!("build_window/widgets.rs");

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: Rc<RefCell<Option<RcPathFn>>>,
) {
    let sub_pref = Rc::new(RefCell::new(db::load_sub()));
    let video_pref = Rc::new(RefCell::new(db::load_video()));
    let reapply_60 = VideoReapply60 { vp: Rc::clone(&video_pref), app: app.clone() };
    let w = build_widgets(app, player, &video_pref, &sub_pref);

    let bar_show = Rc::new(Cell::new(true));
    let nav_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let cur_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let ptr_in_gl = Rc::new(Cell::new(false));
    let motion_squelch = Rc::new(Cell::new(None::<Instant>));
    let last_cap_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_gl_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_path = Rc::new(RefCell::new(None::<PathBuf>));
    let seek_bar_on = Rc::new(Cell::new(db::load_seek_bar_preview()));
    let sibling_seof = Rc::new(SiblingEofState {
        done: Cell::new(false),
        nav_key: RefCell::new(None),
        nav_can_prev: Cell::new(false),
        nav_can_next: Cell::new(false),
    });
    let exit_after_current = Rc::new(Cell::new(false));
    let fs_restore = Rc::new(RefCell::new(None::<(i32, i32)>));
    // Stops `connect_maximized_notify` from re-calling `fullscreen` in the `maximized && !fullscreen`
    // case right after `unfullscreen` (same event tick as leaving fullscreen).
    let skip_max_to_fs = Rc::new(Cell::new(false));
    let last_unmax = Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H)));
    let win_aspect = Rc::new(Cell::new(None::<f64>));
    let aspect_resize_end_deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let aspect_resize_wired = Rc::new(Cell::new(false));
    let idle_inhib = Rc::new(RefCell::new(None::<u32>));

    w.root.add_top_bar(&w.header);
    header_menubtns_switch([
        w.speed_mbtn.clone(), w.sub_menu.clone(), w.vol_menu.clone(), w.menu_btn.clone(),
    ]);

    wire_popover_shows(player, &w, &sub_pref);

    let (seek_sync, seek_grabbed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let seek_preview = seek_bar_preview::connect(
        &w.seek, &w.seek_adj, Rc::clone(player), Rc::clone(&last_path),
        Rc::clone(&seek_bar_on), seek_grabbed.clone(),
        seek_bar_preview::SeekPreviewCtx { ovl: w.outer_ovl.clone(), bottom: w.bottom.clone() },
    );
    // Container lives on the same GdkSurface — no compositor round-trip on show/hide.
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
        speed_sync: w.speed_sync.clone(), speed_list: w.speed_list.clone(),
        video_pref: Rc::clone(&video_pref), app: app.clone(),
    });
    wire_sub_style_controls(SubStyleCtx {
        player: player.clone(), sub_pref: sub_pref.clone(), gl: w.gl_area.clone(),
        bar_show: bar_show.clone(), recent: w.recent_scrl.clone(), bottom: w.bottom.clone(),
        sub_scale_adj: w.sub_scale_adj.clone(), sub_color_btn: w.sub_color_btn.clone(),
    });

    // Double-tap fullscreen on the video (GLArea = hit target). Use **connect_pressed** and
    // `n_press == 2` — on some stacks `connect_released` does not report `n_press == 2` reliably.
    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    let (win_fs, fr, lu, skip_dbl, rec_dbl) = (
        w.win.clone(), fs_restore.clone(), last_unmax.clone(),
        skip_max_to_fs.clone(), w.recent_scrl.clone(),
    );
    dbl.connect_pressed(move |gest, n_press, _, _| {
        if n_press != 2 || rec_dbl.is_visible() { return; }
        let _ = gest.set_state(gtk::EventSequenceState::Claimed);
        toggle_fullscreen(&win_fs, &fr, &lu, &skip_dbl);
    });
    w.gl_area.add_controller(dbl);

    wire_recent_spacer_fullscreen(
        w.recent_spacers, &w.win, &fs_restore, &last_unmax, &skip_max_to_fs, &w.recent_scrl,
    );

    let want_recent = file_boot.borrow().is_none() && !history::load().is_empty();
    w.recent_scrl.set_visible(want_recent);

    let ch_hide = Rc::new(ChromeBarHide {
        nav: nav_t.clone(), vol: w.vol_menu.clone(), sub: w.sub_menu.clone(),
        speed: w.speed_mbtn.clone(), main: w.menu_btn.clone(), root: w.root.clone(),
        gl: w.gl_area.clone(), bar_show: bar_show.clone(), recent: w.recent_scrl.clone(),
        bottom: w.bottom.clone(), player: player.clone(), squelch: motion_squelch.clone(),
        seek_grabbed: seek_grabbed.clone(),
    });

    let on_video_chrome: Rc<dyn Fn()> = {
        let (root, gl, b, recent, bot, p, chh) = (
            w.root.clone(), w.gl_area.clone(), bar_show.clone(),
            w.recent_scrl.clone(), w.bottom.clone(), player.clone(), Rc::clone(&ch_hide),
        );
        Rc::new(move || {
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
            schedule_bars_autohide(Rc::clone(&chh));
        })
    };
    wire_menu_chrome(Rc::clone(&ch_hide), &w.vol_menu, &w.sub_menu, &w.speed_mbtn, &w.menu_btn);
    wire_play_toggles(&w.play_pause, PlayToggleCtx {
        app: app.clone(), player: player.clone(), video_pref: Rc::clone(&video_pref),
        win: w.win.clone(), gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        win_aspect: win_aspect.clone(), sub_menu: Some(w.sub_menu.clone()),
        play_pause: w.play_pause.clone(),
    });
    let browse_chrome: Rc<dyn Fn()> = {
        let (root, gl, b, recent, bot, p, nav) = (
            w.root.clone(), w.gl_area.clone(), bar_show.clone(),
            w.recent_scrl.clone(), w.bottom.clone(), player.clone(), nav_t.clone(),
        );
        Rc::new(move || {
            if let Some(id) = nav.borrow_mut().take() { id.remove(); }
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
        })
    };
    let on_open = make_on_open_handler(OpenHandlerCtx {
        player: player.clone(), win: w.win.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), last_path: last_path.clone(),
        on_start: on_video_chrome.clone(), on_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(&win_aspect), reapply_60: reapply_60.clone(),
        sub_menu: w.sub_menu.clone(),
    });
    *on_open_slot.borrow_mut() = Some(on_open.clone());

    wire_sibling_nav_buttons(SiblingNavCtx {
        btn_prev: w.sibling_nav.prev_btn.clone(), btn_next: w.sibling_nav.next_btn.clone(),
        player: player.clone(), win: w.win.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), last_path: last_path.clone(),
        on_video_chrome: on_video_chrome.clone(), win_aspect: win_aspect.clone(),
        sibling_seof: sibling_seof.clone(), on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
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

    let on_browse_back = make_browse_back(
        BackToBrowseCtx {
            player: player.clone(), on_open: on_open.clone(),
            on_remove: on_remove.clone(), on_trash: on_trash.clone(),
            recent_backfill: recent_backfill.clone(), last_path: last_path.clone(),
            sibling_seof: sibling_seof.clone(), sibling_nav: w.sibling_nav.clone(),
            win_aspect: win_aspect.clone(), on_browse: browse_chrome.clone(),
            undo_shell: undo_shell.clone(), undo_label: undo_label.clone(),
            undo_btn: undo_btn.clone(), undo_timer: undo_timer.clone(),
            undo_remove_stack: undo_remove_stack.clone(),
        },
        w.win.clone(), w.gl_area.clone(), w.recent_scrl.clone(), w.flow_recent.clone(),
    );

    wire_window_input(WindowInputCtx {
        win: w.win.clone(), root: w.root.clone(), header: w.header.clone(),
        outer_ovl: w.outer_ovl.clone(), ovl: w.ovl.clone(), bottom: w.bottom.clone(),
        gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        app: app.clone(), player: player.clone(), video_pref: Rc::clone(&video_pref),
        bar_show: bar_show.clone(), nav_t: nav_t.clone(), cur_t: cur_t.clone(),
        ptr_in_gl: ptr_in_gl.clone(), motion_squelch: motion_squelch.clone(),
        last_cap_xy: last_cap_xy.clone(), last_gl_xy: last_gl_xy.clone(),
        fs_restore: fs_restore.clone(), skip_max_to_fs: skip_max_to_fs.clone(),
        last_unmax: last_unmax.clone(), ch_hide: Rc::clone(&ch_hide),
        on_browse_back: on_browse_back.clone(),
        on_video_chrome: on_video_chrome.clone(), on_file_loaded: Rc::clone(&on_file_loaded),
        last_path: last_path.clone(), win_aspect: win_aspect.clone(),
        play_pause: w.play_pause.clone(),
    });

    let video_file_actions = wire_video_file_actions(VideoFileActionCtx {
        app: app.clone(), player: player.clone(), recent: w.recent_scrl.clone(),
        on_browse_back: on_browse_back.clone(), undo_timer: undo_timer.clone(),
        undo_remove_stack: undo_remove_stack.clone(), do_commit: do_commit.clone(),
        close_action_cell: Rc::clone(&close_act_for_sync),
        trash_action_cell: Rc::clone(&trash_act_for_sync),
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
    });

    wire_seek_control(
        &w.seek, player, &w.gl_area, seek_sync.clone(), seek_grabbed.clone(),
        w.time_left.clone(), Rc::clone(&video_pref),
    );

    let vol_sync = Rc::new(Cell::new(false));
    wire_volume_controls(VolumeCtx {
        player: player.clone(), recent: w.recent_scrl.clone(), gl: w.gl_area.clone(),
        vol_menu: w.vol_menu.clone(), vol_adj: w.vol_adj.clone(),
        vol_mute_btn: w.vol_mute_btn.clone(), vol_sync: vol_sync.clone(),
    });

    wire_aspect_resize_on_map(
        &w.win, &w.recent_scrl, &win_aspect, &aspect_resize_end_deb, &aspect_resize_wired,
    );

    wire_transport_events(TransportSetup {
        app: app.clone(), player: player.clone(), sub_pref: sub_pref.clone(),
        win: w.win.clone(), gl: w.gl_area.clone(), recent: w.recent_scrl.clone(),
        last_path: last_path.clone(), sibling_seof: sibling_seof.clone(),
        sibling_nav: w.sibling_nav.clone(), exit_after_current: exit_after_current.clone(),
        win_aspect: win_aspect.clone(), idle_inhib: Rc::clone(&idle_inhib),
        on_video_chrome: on_video_chrome.clone(), on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(), bar_show: bar_show.clone(),
        widgets: TransportWidgets {
            play_pause: w.play_pause.clone(), seek: w.seek.clone(), seek_adj: w.seek_adj.clone(),
            seek_sync: seek_sync.clone(), seek_grabbed: seek_grabbed.clone(),
            time_left: w.time_left.clone(), time_right: w.time_right.clone(),
            speed_menu: w.speed_mbtn.clone(), vol_menu: w.vol_menu.clone(),
            vol_adj: w.vol_adj.clone(), vol_mute: w.vol_mute_btn.clone(),
            vol_sync: vol_sync.clone(),
        },
    });

    wire_final_actions(FinalActionCtx {
        app: app.clone(), win: w.win.clone(), root: w.root.clone(), gl: w.gl_area.clone(),
        recent: w.recent_scrl.clone(), bottom: w.bottom.clone(), player: player.clone(),
        sub_pref: sub_pref.clone(), video_pref: Rc::clone(&video_pref),
        pref_menu: w.pref_menu.clone(), seek_bar_on: Rc::clone(&seek_bar_on),
        last_path: last_path.clone(), on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded), reapply_60: reapply_60.clone(),
        win_aspect: Rc::clone(&win_aspect), bar_show: bar_show.clone(),
        idle_inhib: Rc::clone(&idle_inhib), exit_after_current: exit_after_current.clone(),
    });
}

fn wire_popover_shows(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    w: &WindowWidgets,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) {
    let (p, bx, blk, gla, sec) = (
        player.clone(), w.audio_tracks_box.clone(),
        Rc::clone(&w.audio_tracks_block), w.gl_area.clone(), w.audio_tracks_section.clone(),
    );
    w.vol_pop.connect_show(move |_| {
        let show = audio_tracks::rebuild_popover(&p, &bx, &blk, &gla);
        sec.set_visible(show);
    });
    let sp_pick = sub_pref.clone();
    let sp_off = sub_pref.clone();
    let on_sub_pick: Rc<dyn Fn(&str)> = Rc::new(move |label: &str| {
        let mut s = sp_pick.borrow_mut();
        s.last_sub_label = label.to_string();
        s.sub_off = false;
        db::save_sub(&sp_pick.borrow());
    });
    let on_sub_off: Rc<dyn Fn()> = Rc::new(move || {
        sp_off.borrow_mut().sub_off = true;
        db::save_sub(&sp_off.borrow());
    });
    let (p2, bx2, blk2, gla2, sec2) = (
        player.clone(), w.sub_tracks_box.clone(),
        Rc::clone(&w.sub_tracks_block), w.gl_area.clone(), w.sub_tracks_section.clone(),
    );
    w.sub_pop.connect_show(move |_| {
        let show = sub_tracks::rebuild_popover(
            &p2, &bx2, &blk2, &gla2,
            Some(Rc::clone(&on_sub_pick)), Some(Rc::clone(&on_sub_off)),
        );
        sec2.set_visible(show);
    });
}
