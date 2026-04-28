/// All GTK widgets constructed for the main window, passed to the wiring phase.
struct WindowWidgets {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    outer_ovl: gtk::Overlay,
    ovl: gtk::Overlay,
    gl_area: gtk::GLArea,
    bottom: gtk::Box,
    play_pause: gtk::Button,
    sibling_nav: SiblingNavUi,
    menu_btn: gtk::MenuButton,
    vol_menu: gtk::MenuButton,
    sub_menu: gtk::MenuButton,
    speed_mbtn: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_sync: Rc<Cell<bool>>,
    seek: gtk::Scale,
    seek_adj: gtk::Adjustment,
    time_left: gtk::Label,
    time_right: gtk::Label,
    vol_adj: gtk::Adjustment,
    vol_mute_btn: gtk::ToggleButton,
    audio_tracks_box: gtk::Box,
    audio_tracks_block: Rc<Cell<bool>>,
    audio_tracks_section: gtk::Box,
    sub_tracks_box: gtk::Box,
    sub_tracks_block: Rc<Cell<bool>>,
    sub_tracks_section: gtk::Box,
    sub_scale_adj: gtk::Adjustment,
    sub_color_btn: gtk::ColorDialogButton,
    vol_pop: gtk::Popover,
    sub_pop: gtk::Popover,
    pref_menu: gio::Menu,
    recent_scrl: gtk::ScrolledWindow,
    flow_recent: gtk::Box,
    recent_spacers: [gtk::Box; 4],
    undo_bar: crate::recent_view::UndoBar,
}

fn build_widgets(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
) -> WindowWidgets {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_WIN_TITLE)
        .icon_name(APP_ID)
        .default_width(WIN_INIT_W)
        .default_height(WIN_INIT_H)
        .css_classes(["rp-win"])
        .build();

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
        vol_adj, vol_mute_btn, audio_tracks_block, audio_tracks_box, audio_tracks_section,
        vol_pop, vol_menu, sub_tracks_block, sub_tracks_box, sub_tracks_section,
        sub_scale_adj, sub_color_btn, sub_pop, sub_menu,
    } = build_header_popovers(sub_pref);

    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    let SpeedMenuResult { speed_mbtn, speed_list, speed_sync } =
        build_speed_menu(player, &gl_area, video_pref, app);

    let menu_btn = build_menu_button(menu);

    let root = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    header.pack_end(&menu_btn);
    header.pack_end(&vol_menu);
    header.pack_end(&sub_menu);
    header.pack_end(&speed_mbtn);

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

    let bottom = build_bottom_bar(&wrap_prev, &play_pause, &wrap_next, &time_left, &seek, &time_right);
    let ovl = build_video_overlay(&gl_area);
    let outer_ovl = gtk::Overlay::new();

    let (recent_scrl, flow_recent, recent_spacers, undo_bar) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);

    WindowWidgets {
        win, root, header, outer_ovl, ovl, gl_area, bottom, play_pause, sibling_nav,
        menu_btn, vol_menu, sub_menu, speed_mbtn, speed_list, speed_sync,
        seek, seek_adj, time_left, time_right,
        vol_adj, vol_mute_btn,
        audio_tracks_box, audio_tracks_block, audio_tracks_section,
        sub_tracks_box, sub_tracks_block, sub_tracks_section,
        sub_scale_adj, sub_color_btn,
        vol_pop, sub_pop, pref_menu,
        recent_scrl, flow_recent, recent_spacers, undo_bar,
    }
}

fn build_menu_button(menu: gio::Menu) -> gtk::MenuButton {
    let mb = gtk::MenuButton::new();
    mb.set_icon_name("open-menu-symbolic");
    mb.set_tooltip_text(Some("Main menu"));
    mb.set_menu_model(Some(&menu));
    let mb2 = mb.clone();
    mb.connect_notify_local(Some("popover"), move |b, _| {
        if let Some(p) = b.popover() { header_popover_non_modal(&p); }
    });
    mb.connect_active_notify(move |b| {
        if b.is_active() {
            if let Some(p) = b.popover() { header_popover_non_modal(&p); }
        }
    });
    if let Some(p) = mb2.popover() { header_popover_non_modal(&p); }
    mb
}

fn build_bottom_bar(
    wrap_prev: &gtk::Box,
    play_pause: &gtk::Button,
    wrap_next: &gtk::Box,
    time_left: &gtk::Label,
    seek: &gtk::Scale,
    time_right: &gtk::Label,
) -> gtk::Box {
    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bottom.add_css_class("rp-bottom");
    bottom.set_vexpand(false);
    play_pause.set_valign(gtk::Align::Center);
    wrap_prev.set_valign(gtk::Align::Center);
    wrap_next.set_valign(gtk::Align::Center);
    bottom.append(wrap_prev);
    bottom.append(play_pause);
    bottom.append(wrap_next);
    bottom.append(time_left);
    bottom.append(seek);
    bottom.append(time_right);
    let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
    close_btn.set_tooltip_text(Some("Close Video (Ctrl+W)"));
    close_btn.add_css_class("flat");
    close_btn.set_valign(gtk::Align::Center);
    close_btn.set_action_name(Some("app.close-video"));
    close_btn.set_margin_start(4);
    bottom.append(&close_btn);
    bottom
}

fn build_video_overlay(gl_area: &gtk::GLArea) -> gtk::Overlay {
    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(gl_area));
    ovl
}
