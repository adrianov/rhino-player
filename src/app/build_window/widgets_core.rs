/// Main application window shell (Chrome, CSD, default size).
fn build_main_application_window(app: &adw::Application) -> adw::ApplicationWindow {
    adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_WIN_TITLE)
        .icon_name(APP_ID)
        .default_width(WIN_INIT_W)
        .default_height(WIN_INIT_H)
        .css_classes(["rp-win"])
        .build()
}

struct PlaybackChromeRow {
    play_pause: gtk::Button,
    sibling_nav: SiblingNavUi,
}

/// Bottom-bar play control and prev/next sibling navigation (wrapped for hit targets).
fn build_playback_chrome_row() -> PlaybackChromeRow {
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

    PlaybackChromeRow {
        play_pause,
        sibling_nav,
    }
}

/// libmpv render target. Linux uses GTK GL; macOS treats this as a transparent
/// sizing placeholder above the native CAOpenGLLayer (`mpv_embed::macos_video_attach`).
fn build_gl_video_area() -> gtk::GLArea {
    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    if cfg!(target_os = "macos") {
        gl_area.add_css_class("rp-gl-native");
    }
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);
    gl_area.set_can_focus(false);
    gl_area.set_focus_on_click(false);
    gl_area
}

struct SeekTimeLabels {
    seek_adj: gtk::Adjustment,
    seek: gtk::Scale,
    time_left: gtk::Label,
    time_right: gtk::Label,
}

fn build_seek_and_time_row() -> SeekTimeLabels {
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

    SeekTimeLabels {
        seek_adj,
        seek,
        time_left,
        time_right,
    }
}

struct ToolbarHeaderShell {
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    fs_clock: gtk::Label,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

/// Toolbar chrome row (menus, clocks); packs header end slots — Linux includes main menu.
fn build_toolbar_header_shell(
    menu_btn: &gtk::MenuButton,
    vol_menu: &gtk::MenuButton,
    sub_menu: &gtk::MenuButton,
    smooth_btn: &gtk::Button,
    speed_mbtn: &gtk::MenuButton,
    blackout_btn: &gtk::Button,
) -> ToolbarHeaderShell {
    let fs_clock = gtk::Label::new(None);
    fs_clock.add_css_class("rp-fs-clock");
    fs_clock.set_valign(gtk::Align::Center);
    fs_clock.set_tooltip_text(Some("Local time"));
    fs_clock.set_visible(false);

    let root = adw::ToolbarView::new();
    root.add_css_class("rp-toolbar");
    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    header.set_height_request(34);
    header.set_size_request(-1, 34);
    #[cfg(not(target_os = "macos"))]
    header.pack_end(menu_btn);
    header.pack_end(vol_menu);
    header.pack_end(sub_menu);
    header.pack_end(smooth_btn);
    header.pack_end(speed_mbtn);
    header.pack_end(blackout_btn);
    header.pack_end(&fs_clock);

    #[cfg(target_os = "macos")]
    let hdr_title_mirror = {
        let lab = Rc::new(gtk::Label::new(Some(APP_WIN_TITLE)));
        lab.add_css_class("title");
        lab.set_valign(gtk::Align::Center);
        lab.set_single_line_mode(true);
        lab.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        header.set_title_widget(Some(lab.as_ref()));
        header.set_show_title(true);
        Some(Rc::clone(&lab))
    };
    #[cfg(not(target_os = "macos"))]
    let hdr_title_mirror: Option<Rc<gtk::Label>> = None;

    #[cfg(target_os = "macos")]
    std::hint::black_box(menu_btn);

    ToolbarHeaderShell {
        root,
        header,
        fs_clock,
        hdr_title_mirror,
    }
}
