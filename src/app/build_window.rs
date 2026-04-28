include!("build_window/app_menus.rs");
include!("build_window/aspect_resize.rs");
include!("build_window/header_popovers.rs");
include!("build_window/sibling_nav_buttons.rs");
include!("build_window/speed_menu.rs");
include!("build_window/volume_wiring.rs");

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: Rc<RefCell<Option<RcPathFn>>>,
) {
    let sub_pref = Rc::new(RefCell::new(db::load_sub()));
    let video_pref = Rc::new(RefCell::new(db::load_video()));
    let reapply_60 = VideoReapply60 {
        vp: Rc::clone(&video_pref),
        app: app.clone(),
    };
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_WIN_TITLE)
        .icon_name(APP_ID)
        .default_width(WIN_INIT_W)
        .default_height(WIN_INIT_H)
        .css_classes(["rp-win"])
        .build();
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
    let root = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    let play_pause = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_pause.add_css_class("flat");
    play_pause.add_css_class("rpb-play");
    play_pause.set_tooltip_text(Some("Play (Space)"));
    play_pause.set_sensitive(false);
    let btn_prev = gtk::Button::from_icon_name("go-previous-symbolic");
    btn_prev.add_css_class("flat");
    btn_prev.add_css_class("rpb-prev");
    btn_prev.set_sensitive(false);
    let wrap_prev = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap_prev.set_can_target(true);
    wrap_prev.append(&btn_prev);
    let btn_next = gtk::Button::from_icon_name("go-next-symbolic");
    btn_next.add_css_class("flat");
    btn_next.add_css_class("rpb-next");
    btn_next.set_sensitive(false);
    let wrap_next = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap_next.set_can_target(true);
    wrap_next.append(&btn_next);
    let sibling_nav = SiblingNavUi::new(&btn_prev, &btn_next, &wrap_prev, &wrap_next);
    let (menu, pref_menu) = build_app_menus();
    let HeaderPopovers {
        vol_adj,
        vol_mute_btn,
        audio_tracks_block,
        audio_tracks_box,
        audio_tracks_section,
        vol_pop,
        vol_menu,
        sub_tracks_block,
        sub_tracks_box,
        sub_tracks_section,
        sub_scale_adj,
        sub_color_btn,
        sub_pop,
        sub_menu,
    } = build_header_popovers(&sub_pref);

    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    // speed_sync and speed_list handler wired inside build_speed_menu
    let SpeedMenuResult { speed_mbtn, speed_list, speed_sync } =
        build_speed_menu(player, &gl_area, &video_pref, app);

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some("Main menu"));
    menu_btn.set_menu_model(Some(&menu));
    {
        let mb = menu_btn.clone();
        menu_btn.connect_notify_local(Some("popover"), move |b, _| {
            if let Some(p) = b.popover() {
                header_popover_non_modal(&p);
            }
        });
        menu_btn.connect_active_notify(move |b| {
            if b.is_active() {
                if let Some(p) = b.popover() {
                    header_popover_non_modal(&p);
                }
            }
        });
        if let Some(p) = mb.popover() {
            header_popover_non_modal(&p);
        }
    }
    header.pack_end(&menu_btn);
    header.pack_end(&vol_menu);
    header.pack_end(&sub_menu);
    header.pack_end(&speed_mbtn);
    header_menubtns_switch([
        speed_mbtn.clone(),
        sub_menu.clone(),
        vol_menu.clone(),
        menu_btn.clone(),
    ]);

    {
        let p = player.clone();
        let bx = audio_tracks_box.clone();
        let blk = Rc::clone(&audio_tracks_block);
        let gla = gl_area.clone();
        let sec = audio_tracks_section.clone();
        vol_pop.connect_show(move |_| {
            let show = audio_tracks::rebuild_popover(&p, &bx, &blk, &gla);
            sec.set_visible(show);
        });
    }
    {
        let p = player.clone();
        let sp_pick = sub_pref.clone();
        let sp_off = sub_pref.clone();
        let bx = sub_tracks_box.clone();
        let blk = Rc::clone(&sub_tracks_block);
        let gla = gl_area.clone();
        let sec = sub_tracks_section.clone();
        let on_sub_pick: Rc<dyn Fn(&str)> = Rc::new(move |label: &str| {
            {
                let mut s = sp_pick.borrow_mut();
                s.last_sub_label = label.to_string();
                s.sub_off = false;
            }
            db::save_sub(&sp_pick.borrow());
        });
        let on_sub_off: Rc<dyn Fn()> = Rc::new(move || {
            sp_off.borrow_mut().sub_off = true;
            db::save_sub(&sp_off.borrow());
        });
        sub_pop.connect_show(move |_| {
            let show = sub_tracks::rebuild_popover(
                &p,
                &bx,
                &blk,
                &gla,
                Some(Rc::clone(&on_sub_pick)),
                Some(Rc::clone(&on_sub_off)),
            );
            sec.set_visible(show);
        });
    }
    let seek_adj = gtk::Adjustment::new(0.0, 0.0, 1.0, 0.2, 1.0, 0.0);
    let seek = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&seek_adj));
    seek.set_hexpand(true);
    seek.set_draw_value(false);
    seek.set_sensitive(false);
    seek.add_css_class("rp-seek");
    seek.set_size_request(120, 0);
    let time_left = gtk::Label::new(Some("0:00"));
    time_left.add_css_class("rp-time");
    time_left.set_xalign(0.0);
    let time_right = gtk::Label::new(Some("0:00"));
    time_right.set_css_classes(&["rp-time", "rp-time-dim"]);
    time_right.set_xalign(1.0);

    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bottom.add_css_class("rp-bottom");
    bottom.set_vexpand(false);
    play_pause.set_valign(gtk::Align::Center);
    wrap_prev.set_valign(gtk::Align::Center);
    wrap_next.set_valign(gtk::Align::Center);
    bottom.append(&wrap_prev);
    bottom.append(&play_pause);
    bottom.append(&wrap_next);
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);
    {
        let b = gtk::Button::from_icon_name("window-close-symbolic");
        b.set_tooltip_text(Some("Close Video (Ctrl+W)"));
        b.add_css_class("flat");
        b.set_valign(gtk::Align::Center);
        b.set_action_name(Some("app.close-video"));
        b.set_margin_start(4);
        bottom.append(&b);
    }

    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(&gl_area));

    // Wraps the ToolbarView so overlay children are rendered above the bottom bar.
    let outer_ovl = gtk::Overlay::new();

    let (seek_sync, seek_grabbed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let seek_preview = seek_bar_preview::connect(
        &seek,
        &seek_adj,
        Rc::clone(player),
        Rc::clone(&last_path),
        Rc::clone(&seek_bar_on),
        seek_grabbed.clone(),
        seek_bar_preview::SeekPreviewCtx {
            ovl: outer_ovl.clone(),
            bottom: bottom.clone(),
        },
    );
    // Container lives on the same GdkSurface — no compositor round-trip on show/hide.
    outer_ovl.add_overlay(&seek_preview.container);

    let (recent_scrl, flow_recent, sp_empty, undo_bar) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);
    let undo_shell = undo_bar.shell.clone();
    let undo_label = undo_bar.label.clone();
    let undo_btn = undo_bar.undo.clone();

    let close_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));
    let trash_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));

    let on_file_loaded = make_file_loaded_handler(FileLoadedCtx {
        player: player.clone(),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        sibling_nav: sibling_nav.clone(),
        sub_pref: sub_pref.clone(),
        gl: gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: recent_scrl.clone(),
        bottom: bottom.clone(),
        sub_menu: sub_menu.clone(),
        close_action_cell: Rc::clone(&close_act_for_sync),
        trash_action_cell: Rc::clone(&trash_act_for_sync),
        speed_sync: speed_sync.clone(),
        speed_list: speed_list.clone(),
        video_pref: Rc::clone(&video_pref),
        app: app.clone(),
    });
    wire_sub_style_controls(SubStyleCtx {
        player: player.clone(),
        sub_pref: sub_pref.clone(),
        gl: gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: recent_scrl.clone(),
        bottom: bottom.clone(),
        sub_scale_adj: sub_scale_adj.clone(),
        sub_color_btn: sub_color_btn.clone(),
    });

    // Double-tap fullscreen on the video (GLArea = hit target). Use **connect_pressed** and
    // `n_press == 2` on the *second* press (same as pre–skip/notify refactors) — on some stacks
    // `connect_released` does not report `n_press == 2` reliably for leaving fullscreen.
    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    {
        let win_fs = win.clone();
        let fr = fs_restore.clone();
        let lu = last_unmax.clone();
        let skip_dbl = skip_max_to_fs.clone();
        let rec_dbl = recent_scrl.clone();
        dbl.connect_pressed(move |gest, n_press, _, _| {
            if n_press != 2 {
                return;
            }
            if rec_dbl.is_visible() {
                return;
            }
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            toggle_fullscreen(&win_fs, &fr, &lu, &skip_dbl);
        });
    }
    gl_area.add_controller(dbl);

    wire_recent_spacer_fullscreen(
        sp_empty,
        &win,
        &fs_restore,
        &last_unmax,
        &skip_max_to_fs,
        &recent_scrl,
    );

    let want_recent = file_boot.borrow().is_none() && !history::load().is_empty();
    recent_scrl.set_visible(want_recent);

    let ch_hide = Rc::new(ChromeBarHide {
        nav: nav_t.clone(),
        vol: vol_menu.clone(),
        sub: sub_menu.clone(),
        speed: speed_mbtn.clone(),
        main: menu_btn.clone(),
        root: root.clone(),
        gl: gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: recent_scrl.clone(),
        bottom: bottom.clone(),
        player: player.clone(),
        squelch: motion_squelch.clone(),
        seek_grabbed: seek_grabbed.clone(),
    });

    let on_video_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let bot = bottom.clone();
        let p = player.clone();
        let chh = Rc::clone(&ch_hide);
        Rc::new(move || {
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
            schedule_bars_autohide(Rc::clone(&chh));
        })
    };
    wire_menu_chrome(
        Rc::clone(&ch_hide),
        &vol_menu,
        &sub_menu,
        &speed_mbtn,
        &menu_btn,
    );
    wire_play_toggles(
        &play_pause,
        PlayToggleCtx {
            app: app.clone(),
            player: player.clone(),
            video_pref: Rc::clone(&video_pref),
            win: win.clone(),
            gl: gl_area.clone(),
            recent: recent_scrl.clone(),
            last_path: last_path.clone(),
            on_video_chrome: on_video_chrome.clone(),
            win_aspect: win_aspect.clone(),
            sub_menu: Some(sub_menu.clone()),
            play_pause: play_pause.clone(),
        },
    );
    let browse_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let bot = bottom.clone();
        let p = player.clone();
        let nav = nav_t.clone();
        Rc::new(move || {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
        })
    };
    let on_open_vid = on_video_chrome.clone();
    let on_open = make_on_open_handler(OpenHandlerCtx {
        player: player.clone(),
        win: win.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        last_path: last_path.clone(),
        on_start: on_open_vid.clone(),
        on_loaded: Rc::clone(&on_file_loaded),
        win_aspect: Rc::clone(&win_aspect),
        reapply_60: reapply_60.clone(),
        sub_menu: sub_menu.clone(),
    });
    *on_open_slot.borrow_mut() = Some(on_open.clone());

    wire_sibling_nav_buttons(SiblingNavCtx {
        btn_prev: btn_prev.clone(),
        btn_next: btn_next.clone(),
        player: player.clone(),
        win: win.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        last_path: last_path.clone(),
        on_video_chrome: on_open_vid.clone(),
        win_aspect: win_aspect.clone(),
        sibling_seof: sibling_seof.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
    });

    let recent_wiring = wire_recent_undo(RecentUndoCtx {
        player: player.clone(),
        recent: recent_scrl.clone(),
        flow: flow_recent.clone(),
        undo_shell: undo_shell.clone(),
        undo_label: undo_label.clone(),
        undo_btn: undo_btn.clone(),
        undo_close: undo_bar.close.clone(),
        on_open: on_open.clone(),
        want_recent,
    });
    let recent_backfill = recent_wiring.recent_backfill;
    let pending_recent_backfill = recent_wiring.pending_recent_backfill;
    let undo_remove_stack = recent_wiring.undo_remove_stack;
    let undo_timer = recent_wiring.undo_timer;
    let do_commit = recent_wiring.do_commit;
    let on_remove = recent_wiring.on_remove;
    let on_trash = recent_wiring.on_trash;

    wire_window_input(WindowInputCtx {
        win: win.clone(),
        root: root.clone(),
        header: header.clone(),
        outer_ovl: outer_ovl.clone(),
        ovl: ovl.clone(),
        bottom: bottom.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        flow_recent: flow_recent.clone(),
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
        skip_max_to_fs: skip_max_to_fs.clone(),
        last_unmax: last_unmax.clone(),
        ch_hide: Rc::clone(&ch_hide),
        on_open: on_open.clone(),
        on_remove: on_remove.clone(),
        on_trash: on_trash.clone(),
        recent_backfill: recent_backfill.clone(),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        sibling_nav: sibling_nav.clone(),
        on_video_chrome: on_video_chrome.clone(),
        browse_chrome: browse_chrome.clone(),
        win_aspect: win_aspect.clone(),
        play_pause: play_pause.clone(),
        undo_shell: undo_shell.clone(),
        undo_label: undo_label.clone(),
        undo_btn: undo_btn.clone(),
        undo_timer: undo_timer.clone(),
        undo_remove_stack: undo_remove_stack.clone(),
    });

    let video_file_actions = wire_video_file_actions(VideoFileActionCtx {
        app: app.clone(),
        player: player.clone(),
        win: win.clone(),
        recent: recent_scrl.clone(),
        flow_recent: flow_recent.clone(),
        gl: gl_area.clone(),
        on_open: on_open.clone(),
        on_remove: on_remove.clone(),
        on_trash: on_trash.clone(),
        recent_backfill: recent_backfill.clone(),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        sibling_nav: sibling_nav.clone(),
        browse_chrome: browse_chrome.clone(),
        win_aspect: win_aspect.clone(),
        undo_shell: undo_shell.clone(),
        undo_label: undo_label.clone(),
        undo_btn: undo_btn.clone(),
        undo_timer: undo_timer.clone(),
        undo_remove_stack: undo_remove_stack.clone(),
        do_commit: do_commit.clone(),
        close_action_cell: Rc::clone(&close_act_for_sync),
        trash_action_cell: Rc::clone(&trash_act_for_sync),
    });

    wire_mpv_realize(MpvRealizeCtx {
        player: player.clone(),
        sub_pref: sub_pref.clone(),
        video_pref: Rc::clone(&video_pref),
        app: app.clone(),
        win: win.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        bar_show: bar_show.clone(),
        bottom: bottom.clone(),
        last_path: last_path.clone(),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        file_boot: Rc::clone(&file_boot),
        win_aspect: Rc::clone(&win_aspect),
        reapply_60: reapply_60.clone(),
        pending_recent_backfill: pending_recent_backfill.clone(),
        close_video: video_file_actions.close_video,
        move_to_trash: video_file_actions.move_to_trash,
    });

    wire_seek_control(&seek, player, &gl_area, seek_sync.clone(), seek_grabbed.clone(), time_left.clone(), Rc::clone(&video_pref));

    let vol_sync = Rc::new(Cell::new(false));
    wire_volume_controls(VolumeCtx {
        player: player.clone(),
        recent: recent_scrl.clone(),
        gl: gl_area.clone(),
        vol_menu: vol_menu.clone(),
        vol_adj: vol_adj.clone(),
        vol_mute_btn: vol_mute_btn.clone(),
        vol_sync: vol_sync.clone(),
    });

    wire_aspect_resize_on_map(
        &win,
        &recent_scrl,
        &win_aspect,
        &aspect_resize_end_deb,
        &aspect_resize_wired,
    );

    wire_transport_events(TransportSetup {
        app: app.clone(),
        player: player.clone(),
        sub_pref: sub_pref.clone(),
        win: win.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        sibling_nav: sibling_nav.clone(),
        exit_after_current: exit_after_current.clone(),
        win_aspect: win_aspect.clone(),
        idle_inhib: Rc::clone(&idle_inhib),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
        bar_show: bar_show.clone(),
        widgets: TransportWidgets {
            play_pause: play_pause.clone(),
            seek: seek.clone(),
            seek_adj: seek_adj.clone(),
            seek_sync: seek_sync.clone(),
            seek_grabbed: seek_grabbed.clone(),
            time_left: time_left.clone(),
            time_right: time_right.clone(),
            speed_menu: speed_mbtn.clone(),
            vol_menu: vol_menu.clone(),
            vol_adj: vol_adj.clone(),
            vol_mute: vol_mute_btn.clone(),
            vol_sync: vol_sync.clone(),
        },
    });

    wire_final_actions(FinalActionCtx {
        app: app.clone(),
        win: win.clone(),
        root: root.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        bottom: bottom.clone(),
        player: player.clone(),
        sub_pref: sub_pref.clone(),
        video_pref: Rc::clone(&video_pref),
        pref_menu: pref_menu.clone(),
        seek_bar_on: Rc::clone(&seek_bar_on),
        last_path: last_path.clone(),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
        win_aspect: Rc::clone(&win_aspect),
        bar_show: bar_show.clone(),
        idle_inhib: Rc::clone(&idle_inhib),
        exit_after_current: exit_after_current.clone(),
    });
}
