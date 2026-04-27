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
        stall: Cell::new((0.0, 0u8)),
        nav_key: RefCell::new(None),
        nav_can_prev: Cell::new(false),
        nav_can_next: Cell::new(false),
    });
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
    wrap_prev.append(&btn_prev);
    wrap_prev.set_tooltip_text(Some("Previous file in folder"));
    btn_prev.set_has_tooltip(false);
    let btn_next = gtk::Button::from_icon_name("go-next-symbolic");
    btn_next.add_css_class("flat");
    btn_next.add_css_class("rpb-next");
    btn_next.set_sensitive(false);
    let wrap_next = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap_next.append(&btn_next);
    wrap_next.set_tooltip_text(Some("Next file in folder"));
    btn_next.set_has_tooltip(false);
    let pref_menu = gio::Menu::new();
    pref_menu.append(Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"));
    pref_menu.append(
        Some("Choose VapourSynth script (.vpy)…"),
        Some("app.choose-vs"),
    );

    let menu = gio::Menu::new();
    menu.append(Some("Open video…"), Some("app.open"));
    menu.append(Some("Close video"), Some("app.close-video"));
    menu.append(Some("Move to Trash"), Some("app.move-to-trash"));
    menu.append_submenu(Some("Preferences"), &pref_menu);
    menu.append(Some("About Rhino Player"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let vol_adj = gtk::Adjustment::new(100.0, 0.0, 100.0, 1.0, 5.0, 0.0);
    let vol_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&vol_adj));
    vol_scale.set_draw_value(false);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(240, -1);
    vol_scale.set_valign(gtk::Align::Center);
    vol_scale.set_tooltip_text(Some("Volume"));
    vol_scale.add_css_class("rp-vol");
    let vol_mute_btn = gtk::ToggleButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .valign(gtk::Align::Center)
        .vexpand(false)
        .tooltip_text("Mute")
        .build();
    vol_mute_btn.add_css_class("flat");
    vol_mute_btn.add_css_class("circular");
    let vol_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    vol_row.set_valign(gtk::Align::Center);
    vol_row.set_size_request(300, -1);
    vol_row.append(&vol_mute_btn);
    vol_row.append(&vol_scale);

    let audio_tracks_block = Rc::new(Cell::new(false));
    let audio_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    audio_tracks_box.set_margin_top(2);
    let audio_tracks_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(400)
        .max_content_height(480)
        .child(&audio_tracks_box)
        .build();
    let audio_tracks_section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    audio_tracks_section.append(&audio_tracks_scrl);
    audio_tracks_section.set_visible(false);
    let sound_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    sound_col.add_css_class("rp-popover-box");
    sound_col.append(&vol_row);
    sound_col.append(&audio_tracks_section);
    let vol_pop = gtk::Popover::new();
    vol_pop.add_css_class("rp-header-popover");
    vol_pop.set_child(Some(&sound_col));
    header_popover_non_modal(&vol_pop);
    let vol_menu = gtk::MenuButton::new();
    vol_menu.set_icon_name("audio-volume-high-symbolic");
    vol_menu.set_tooltip_text(Some("Volume and mute; audio track list if several tracks"));
    vol_menu.set_popover(Some(&vol_pop));
    vol_menu.add_css_class("flat");

    let sp_init = sub_pref.borrow().clone();
    let sub_tracks_block = Rc::new(Cell::new(false));
    let sub_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    sub_tracks_box.set_margin_top(2);
    let sub_tracks_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(360)
        .max_content_height(280)
        .child(&sub_tracks_box)
        .build();
    let sub_tracks_section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sub_tracks_section.append(&sub_tracks_scrl);
    sub_tracks_section.set_visible(false);

    let sub_scale_adj = gtk::Adjustment::new(sp_init.scale, 0.3, 2.0, 0.05, 0.1, 0.0);
    let sub_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&sub_scale_adj));
    sub_scale.set_draw_value(true);
    sub_scale.set_digits(2);
    sub_scale.set_hexpand(true);
    sub_scale.set_size_request(240, -1);
    sub_scale.set_tooltip_text(Some("Subtitle size (mpv sub-scale)"));

    let sub_color_btn = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    sub_color_btn.set_rgba(&sub_prefs::u32_to_rgba(sp_init.color));
    sub_color_btn.set_tooltip_text(Some("Subtitle text color"));

    let sub_opts = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let sub_size_label = gtk::Label::new(Some("Size"));
    sub_size_label.set_xalign(0.0);
    sub_size_label.add_css_class("caption");
    sub_opts.append(&sub_size_label);
    sub_opts.append(&sub_scale);
    let sub_color_label = gtk::Label::new(Some("Text color"));
    sub_color_label.set_xalign(0.0);
    sub_color_label.add_css_class("caption");
    sub_opts.append(&sub_color_label);
    sub_opts.append(&sub_color_btn);

    let sub_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    sub_col.add_css_class("rp-popover-box");
    sub_col.append(&sub_tracks_section);
    sub_col.append(&sub_opts);

    let sub_pop = gtk::Popover::new();
    sub_pop.add_css_class("rp-header-popover");
    sub_pop.set_child(Some(&sub_col));
    header_popover_non_modal(&sub_pop);
    let sub_menu = gtk::MenuButton::new();
    sub_menu.set_icon_name("media-view-subtitles-symbolic");
    sub_menu.set_tooltip_text(Some("Subtitles: tracks and style"));
    sub_menu.set_popover(Some(&sub_pop));
    sub_menu.add_css_class("flat");
    sub_menu.set_visible(false);

    let speed_list = gtk::ListBox::new();
    speed_list.set_activate_on_single_click(true);
    speed_list.add_css_class("rich-list");
    for s in &["1.0×", "1.5×", "2.0×"] {
        let row = gtk::ListBoxRow::new();
        let lab = gtk::Label::new(Some(*s));
        lab.set_halign(gtk::Align::Start);
        lab.set_margin_start(10);
        lab.set_margin_end(10);
        lab.set_margin_top(6);
        lab.set_margin_bottom(6);
        row.set_child(Some(&lab));
        speed_list.append(&row);
    }
    let speed_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    speed_col.add_css_class("rp-popover-box");
    speed_col.append(&speed_list);
    let speed_pop = gtk::Popover::new();
    speed_pop.add_css_class("rp-header-popover");
    speed_pop.set_child(Some(&speed_col));
    header_popover_non_modal(&speed_pop);
    let speed_mbtn = gtk::MenuButton::new();
    speed_mbtn.set_icon_name("speedometer-symbolic");
    speed_mbtn.set_tooltip_text(Some("Playback speed"));
    speed_mbtn.set_popover(Some(&speed_pop));
    speed_mbtn.set_sensitive(false);
    speed_mbtn.add_css_class("flat");

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

    let gl_area = gtk::GLArea::new();
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
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    wire_play_toggles(&play_pause, &gl_area, player);

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
    let speed_sync = Rc::new(Cell::new(false));
    let vp_speed = Rc::clone(&video_pref);
    let app_speed = app.clone();
    {
        let p = player.clone();
        let glr = gl_area.clone();
        let sy = speed_sync.clone();
        let smb = speed_mbtn.clone();
        let vp = Rc::clone(&vp_speed);
        let ap = app_speed.clone();
        speed_list.connect_row_activated(move |list2, row| {
            if sy.get() {
                return;
            }
            let i: u32 = (0i32..3)
                .find(|&ix| list2.row_at_index(ix).is_some_and(|r| r == *row))
                .unwrap_or(0) as u32;
            let v = playback_speed::value_at(i);
            if let Some(b) = p.borrow().as_ref() {
                let _ = b.mpv.set_property("speed", v);
                glr.queue_render();
                // Defer [vf] rebuild: libmpv can still report the old [speed] on the same GTK tick as
                // [set_property]; [mvtools_vf_eligible] + [add_smooth_60] must see 1.0× when returning from 1.5/2.0.
                let bref = p.clone();
                let vp2 = Rc::clone(&vp);
                let ap2 = ap.clone();
                let vh = v;
                let _ = glib::idle_add_local_once(move || {
                    if let Some(pl) = bref.borrow().as_ref() {
                        let mut g = vp2.borrow_mut();
                        if video_pref::refresh_smooth_for_playback_speed(&pl.mpv, &mut g, Some(vh))
                        {
                            sync_smooth_60_to_off(&ap2);
                        }
                    }
                });
            }
            smb.set_active(false);
        });
    }
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);
    {
        let b = gtk::Button::from_icon_name("window-close-symbolic");
        b.set_tooltip_text(Some("Close video (Ctrl+W)"));
        b.add_css_class("flat");
        b.set_valign(gtk::Align::Center);
        b.set_action_name(Some("app.close-video"));
        b.set_margin_start(4);
        bottom.append(&b);
    }

    let seek_state = seek_bar_preview::connect(
        &seek,
        &seek_adj,
        Rc::clone(player),
        Rc::clone(&last_path),
        Rc::clone(&seek_bar_on),
    );

    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(&gl_area));

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

    {
        let p = player.clone();
        let w = win.clone();
        let gla = gl_area.clone();
        let rec = recent_scrl.clone();
        let lp = last_path.clone();
        let ovid = on_open_vid.clone();
        let wa = win_aspect.clone();
        let seof = sibling_seof.clone();
        let ol = Rc::clone(&on_file_loaded);
        btn_prev.connect_clicked(glib::clone!(
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            gla,
            #[strong]
            rec,
            #[strong]
            lp,
            #[strong]
            ovid,
            #[strong]
            wa,
            #[strong]
            seof,
            #[strong]
            ol,
            #[strong]
            reapply_60,
            move |_| {
                let g = p.borrow();
                let Some(pl) = g.as_ref() else {
                    return;
                };
                let cur = local_file_from_mpv(&pl.mpv).or_else(|| lp.borrow().clone());
                let Some(cur) = cur.filter(|c| c.is_file()) else {
                    return;
                };
                let Some(np) = sibling_advance::prev_before_current(&cur) else {
                    return;
                };
                seof.done.set(false);
                seof.stall.set((0.0, 0));
                drop(g);
                let o = LoadOpts {
                    record: true,
                    play_on_start: true,
                    last_path: Rc::clone(&lp),
                    on_start: Some(Rc::clone(&ovid)),
                    win_aspect: Rc::clone(&wa),
                    on_loaded: Some(Rc::clone(&ol)),
                    reapply_60: Some(reapply_60.clone()),
                };
                if let Err(e) = try_load(&np, &p, &w, &gla, &rec, &o) {
                    eprintln!("[rhino] previous: {e}");
                }
            }
        ));
        let ol2 = Rc::clone(&on_file_loaded);
        btn_next.connect_clicked(glib::clone!(
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            gla,
            #[strong]
            rec,
            #[strong]
            lp,
            #[strong]
            ovid,
            #[strong]
            wa,
            #[strong]
            seof,
            #[strong]
            ol2,
            #[strong]
            reapply_60,
            move |_| {
                let g = p.borrow();
                let Some(pl) = g.as_ref() else {
                    return;
                };
                let cur = local_file_from_mpv(&pl.mpv).or_else(|| lp.borrow().clone());
                let Some(cur) = cur.filter(|c| c.is_file()) else {
                    return;
                };
                let Some(np) = sibling_advance::next_after_eof(&cur) else {
                    return;
                };
                seof.done.set(false);
                seof.stall.set((0.0, 0));
                drop(g);
                let o = LoadOpts {
                    record: true,
                    play_on_start: true,
                    last_path: Rc::clone(&lp),
                    on_start: Some(Rc::clone(&ovid)),
                    win_aspect: Rc::clone(&wa),
                    on_loaded: Some(Rc::clone(&ol2)),
                    reapply_60: Some(reapply_60.clone()),
                };
                if let Err(e) = try_load(&np, &p, &w, &gla, &rec, &o) {
                    eprintln!("[rhino] next: {e}");
                }
            }
        ));
    }

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
        ovl: ovl.clone(),
        bottom: bottom.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        flow_recent: flow_recent.clone(),
        player: player.clone(),
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
        browse_chrome: browse_chrome.clone(),
        win_aspect: win_aspect.clone(),
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

    // Shared with [start_transport_poll]; prevents programmatic seek updates from re-seeking mpv.
    let seek_sync = Rc::new(Cell::new(false));
    let p_seek = player.clone();
    seek.connect_value_changed(glib::clone!(
        #[strong]
        p_seek,
        #[strong]
        seek_sync,
        move |r| {
            if seek_sync.get() {
                return;
            }
            if let Some(b) = p_seek.borrow().as_ref() {
                let s = format!("{:.4}", r.value());
                if b.mpv
                    .command("seek", &[s.as_str(), "absolute+keyframes"])
                    .is_err()
                {
                    let _ = b.mpv.set_property("time-pos", r.value());
                }
            }
        }
    ));

    let vol_sync = Rc::new(Cell::new(false));
    let p_vctl = player.clone();
    let vi = vol_menu.clone();
    let vm = vol_mute_btn.clone();
    let vsx = vol_sync.clone();
    vol_adj.connect_value_changed(glib::clone!(
        #[strong]
        p_vctl,
        #[strong]
        vi,
        #[strong]
        vm,
        #[strong]
        vsx,
        move |a| {
            if vsx.get() {
                return;
            }
            if let Some(b) = p_vctl.borrow().as_ref() {
                let v = a.value();
                let _ = b.mpv.set_property("volume", v);
                if v > 0.5 {
                    let _ = b.mpv.set_property("mute", false);
                }
                let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
                let cur = b.mpv.get_property::<f64>("volume").unwrap_or(v);
                vi.set_icon_name(vol_icon(m, cur));
                vsx.set(true);
                if vm.is_active() != m {
                    vm.set_active(m);
                }
                vm.set_icon_name(vol_mute_pop_icon(m));
                vm.set_tooltip_text(Some(if m { "Unmute" } else { "Mute" }));
                vsx.set(false);
            }
        }
    ));
    let p_mute = player.clone();
    let vi2 = vol_menu.clone();
    let vsx2 = vol_sync.clone();
    vol_mute_btn.connect_toggled(glib::clone!(
        #[strong]
        p_mute,
        #[strong]
        vi2,
        #[strong]
        vsx2,
        move |ch| {
            if vsx2.get() {
                return;
            }
            if let Some(b) = p_mute.borrow().as_ref() {
                let m = ch.is_active();
                let _ = b.mpv.set_property("mute", m);
                let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
                vi2.set_icon_name(vol_icon(m, vol));
                ch.set_icon_name(vol_mute_pop_icon(m));
                ch.set_tooltip_text(Some(if m { "Unmute" } else { "Mute" }));
            }
        }
    ));

    {
        let p = player.clone();
        let r = recent_scrl.clone();
        let vmi = vol_menu.clone();
        let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        sc.set_propagation_phase(gtk::PropagationPhase::Target);
        sc.connect_scroll(glib::clone!(
            #[strong]
            p,
            #[strong]
            r,
            #[strong]
            vmi,
            move |_, _dx, dy| {
                if r.is_visible() {
                    return glib::Propagation::Proceed;
                }
                let g = p.borrow();
                let Some(b) = g.as_ref() else {
                    return glib::Propagation::Proceed;
                };
                let step = if dy.abs() < 0.5 { -dy * 4.0 } else { -dy * 5.0 };
                nudge_mpv_volume(&b.mpv, step);
                let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
                let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
                vmi.set_icon_name(vol_icon(m, vol));
                glib::Propagation::Stop
            }
        ));
        gl_area.add_controller(sc);
    }

    {
        let deb = aspect_resize_end_deb.clone();
        let wired = aspect_resize_wired.clone();
        let w = win.clone();
        let r = recent_scrl.clone();
        let wa = win_aspect.clone();
        w.connect_map(glib::clone!(
            #[strong]
            w,
            #[strong]
            r,
            #[strong]
            wa,
            #[strong]
            deb,
            #[strong]
            wired,
            move |_| {
                if wired.get() {
                    return;
                }
                let on_resize: Rc<dyn Fn()> = Rc::new(glib::clone!(
                    #[strong]
                    deb,
                    #[strong]
                    w,
                    #[strong]
                    r,
                    #[strong]
                    wa,
                    move || schedule_window_aspect_on_resize_end(Rc::clone(&deb), &w, &r, &wa)
                ));
                let Some(n) = w.native() else {
                    return;
                };
                let Some(surf) = n.surface() else {
                    return;
                };
                surf.connect_width_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                surf.connect_height_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                let gw: &gtk::Window = w.upcast_ref();
                gw.connect_default_width_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                gw.connect_default_height_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                wired.set(true);
                if aspect_debug() {
                    eprintln!(
                        "[rhino] aspect: resize-end hooks (GdkSurface + GtkWindow default size)"
                    );
                }
            }
        ));
    }

    start_transport_poll(TransportPollCtx {
        player: player.clone(),
        win: win.clone(),
        gl: gl_area.clone(),
        recent: recent_scrl.clone(),
        last_path: last_path.clone(),
        sibling_seof: sibling_seof.clone(),
        win_aspect: win_aspect.clone(),
        on_video_chrome: on_video_chrome.clone(),
        on_file_loaded: Rc::clone(&on_file_loaded),
        reapply_60: reapply_60.clone(),
        seek_state,
        speed_menu: speed_mbtn.clone(),
        seek: seek.clone(),
        seek_adj: seek_adj.clone(),
        seek_sync: seek_sync.clone(),
        time_left: time_left.clone(),
        time_right: time_right.clone(),
        play_pause: play_pause.clone(),
        wrap_prev: wrap_prev.clone(),
        wrap_next: wrap_next.clone(),
        btn_prev: btn_prev.clone(),
        btn_next: btn_next.clone(),
        vol_menu: vol_menu.clone(),
        vol_adj: vol_adj.clone(),
        vol_mute: vol_mute_btn.clone(),
        vol_sync: vol_sync.clone(),
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
    });
}
