include!("widgets_core.rs");

/// All GTK widgets constructed for the main window, passed to the wiring phase.
struct WindowWidgets {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    outer_ovl: gtk::Overlay,
    video_handle: gtk::WindowHandle,
    gl_area: gtk::GLArea,
    bottom: gtk::Box,
    play_pause: gtk::Button,
    sibling_nav: SiblingNavUi,
    menu_btn: gtk::MenuButton,
    vol_menu: gtk::MenuButton,
    sub_menu: gtk::MenuButton,
    speed_mbtn: gtk::MenuButton,
    speed_readout: gtk::Label,
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
    /// Unused empty model on Linux; macOS receives the hierarchical menubar menu.
    main_menu: gio::Menu,
    pref_menu: gio::Menu,
    recent_scrl: gtk::Box,
    flow_recent: gtk::Box,
    recent_spacers: [gtk::Box; 4],
    undo_bar: crate::recent_view::UndoBar,
    /// Local wall-clock readout; visible only in fullscreen (`docs/features/17-window-behavior.md`).
    fs_clock: gtk::Label,
    /// macOS GTK: optional label in [`adw::HeaderBar::title_widget`] so double-click toggles fullscreen.
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

fn build_widgets(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    exit_after_current: Rc<Cell<bool>>,
) -> WindowWidgets {
    #[cfg(target_os = "macos")]
    std::hint::black_box(exit_after_current.clone());

    let win = build_main_application_window(app);
    let PlaybackChromeRow { play_pause, sibling_nav } = build_playback_chrome_row();

    let (discard_menu_placeholder, pref_menu, menubar_model) = build_app_menus();
    drop(discard_menu_placeholder);
    let HeaderPopovers {
        vol_adj, vol_mute_btn, audio_tracks_block, audio_tracks_box, audio_tracks_section,
        vol_pop, vol_menu, sub_tracks_block, sub_tracks_box, sub_tracks_section,
        sub_scale_adj, sub_color_btn, sub_pop, sub_menu,
    } = build_header_popovers(sub_pref);

    let gl_area = build_gl_video_area();
    let SpeedMenuResult { speed_readout, speed_mbtn, speed_list, speed_sync } =
        build_speed_menu(player, &gl_area, video_pref, app);
    let speed_pack = gtk::Box::new(gtk::Orientation::Vertical, 0);
    speed_pack.add_css_class("rp-speed-cluster");
    speed_pack.set_valign(gtk::Align::Center);
    speed_pack.set_hexpand(false);
    speed_pack.set_vexpand(false);
    speed_pack.append(&speed_mbtn);
    speed_pack.append(&speed_readout);

    let menu_btn = {
        #[cfg(not(target_os = "macos"))]
        {
            build_linux_main_menu_button(app, &pref_menu, &exit_after_current)
        }
        #[cfg(target_os = "macos")]
        {
            gtk::MenuButton::new()
        }
    };

    let ToolbarHeaderShell {
        root,
        header,
        fs_clock,
        hdr_title_mirror,
    } = build_toolbar_header_shell(&menu_btn, &vol_menu, &sub_menu, &speed_pack);

    let SeekTimeLabels {
        seek_adj,
        seek,
        time_left,
        time_right,
    } = build_seek_and_time_row();

    let bottom = build_bottom_bar(
        &sibling_nav.prev_wrap,
        &play_pause,
        &sibling_nav.next_wrap,
        &time_left,
        &seek,
        &time_right,
    );
    let ovl = build_video_overlay(&gl_area);
    let video_handle = gtk::WindowHandle::new();
    video_handle.set_child(Some(&ovl));
    let outer_ovl = gtk::Overlay::new();

    let (recent_scrl, flow_recent, recent_spacers, undo_bar) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);

    WindowWidgets {
        win, root, header, outer_ovl, video_handle, gl_area, bottom, play_pause, sibling_nav,
        menu_btn, vol_menu, sub_menu, speed_mbtn, speed_readout, speed_list, speed_sync,
        seek, seek_adj, time_left, time_right,
        vol_adj, vol_mute_btn,
        audio_tracks_box, audio_tracks_block, audio_tracks_section,
        sub_tracks_box, sub_tracks_block, sub_tracks_section,
        sub_scale_adj, sub_color_btn,
        vol_pop, sub_pop, main_menu: menubar_model, pref_menu,
        recent_scrl, flow_recent, recent_spacers, undo_bar,
        fs_clock,
        hdr_title_mirror,
    }
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

fn build_video_overlay(child: &gtk::GLArea) -> gtk::Overlay {
    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(child));
    ovl
}