struct ToolbarHeaderShell {
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    fs_clock: gtk::Label,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    /// Keeps header [`gtk::SizeGroup`] alive so face `MenuButton`s match `Button` height.
    _header_btn_heights: gtk::SizeGroup,
}

/// Equalize header control height to the tallest peer (smooth / menu), without shrinking them.
fn wire_header_btn_heights(
    group: &gtk::SizeGroup,
    #[cfg(not(target_os = "macos"))] menu_btn: &gtk::MenuButton,
    vol_menu: &gtk::MenuButton,
    sub_menu: &gtk::MenuButton,
    smooth_btn: &gtk::Button,
    speed_mbtn: &gtk::MenuButton,
    fill_btn: &gtk::Button,
    blackout_btn: &gtk::Button,
) {
    #[cfg(not(target_os = "macos"))]
    group.add_widget(menu_btn);
    for w in [vol_menu, sub_menu, speed_mbtn] {
        group.add_widget(w);
    }
    for w in [smooth_btn, fill_btn, blackout_btn] {
        group.add_widget(w);
    }
}

/// Toolbar chrome row (menus, clocks); packs header end slots — Linux includes main menu.
fn build_toolbar_header_shell(
    menu_btn: &gtk::MenuButton,
    vol_menu: &gtk::MenuButton,
    sub_menu: &gtk::MenuButton,
    smooth_btn: &gtk::Button,
    speed_mbtn: &gtk::MenuButton,
    fill_btn: &gtk::Button,
    blackout_btn: &gtk::Button,
) -> ToolbarHeaderShell {
    let (fs_clock, root, header) = new_toolbar_header();
    pack_header_end(
        &header,
        [
            menu_btn.upcast_ref(),
            vol_menu.upcast_ref(),
            sub_menu.upcast_ref(),
            smooth_btn.upcast_ref(),
            speed_mbtn.upcast_ref(),
            fill_btn.upcast_ref(),
            blackout_btn.upcast_ref(),
            fs_clock.upcast_ref(),
        ],
    );

    let header_btn_heights = gtk::SizeGroup::new(gtk::SizeGroupMode::Vertical);
    wire_header_btn_heights(
        &header_btn_heights,
        #[cfg(not(target_os = "macos"))]
        menu_btn,
        vol_menu,
        sub_menu,
        smooth_btn,
        speed_mbtn,
        fill_btn,
        blackout_btn,
    );

    #[cfg(target_os = "macos")]
    let hdr_title_mirror = macos_title_mirror(&header);
    #[cfg(not(target_os = "macos"))]
    let hdr_title_mirror: Option<Rc<gtk::Label>> = None;

    ToolbarHeaderShell {
        root,
        header,
        fs_clock,
        hdr_title_mirror,
        _header_btn_heights: header_btn_heights,
    }
}

/// Fresh fullscreen clock plus the styled [`adw::ToolbarView`] / [`adw::HeaderBar`] shell.
fn new_toolbar_header() -> (gtk::Label, adw::ToolbarView, adw::HeaderBar) {
    let fs_clock = new_fs_clock();
    let root = adw::ToolbarView::new();
    root.add_css_class("rp-toolbar");
    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    header.set_height_request(34);
    header.set_size_request(-1, 34);
    (fs_clock, root, header)
}

fn new_fs_clock() -> gtk::Label {
    let fs_clock = gtk::Label::new(None);
    fs_clock.add_css_class("rp-fs-clock");
    fs_clock.set_valign(gtk::Align::Center);
    fs_clock.set_tooltip_text(Some("Local time"));
    fs_clock.set_visible(false);
    fs_clock
}

/// Packs the toolbar chrome row into header end slots — Linux includes main menu.
fn pack_header_end(header: &adw::HeaderBar, end_widgets: [&gtk::Widget; 8]) {
    // [0] is the Linux main-menu button; macOS uses the system menubar instead.
    #[cfg(target_os = "macos")]
    let skip = 1;
    #[cfg(not(target_os = "macos"))]
    let skip = 0;
    for w in end_widgets.iter().skip(skip) {
        header.pack_end(*w);
    }
}

#[cfg(target_os = "macos")]
fn macos_title_mirror(header: &adw::HeaderBar) -> Option<Rc<gtk::Label>> {
    let lab = Rc::new(gtk::Label::new(Some(APP_WIN_TITLE)));
    lab.add_css_class("title");
    lab.set_valign(gtk::Align::Center);
    lab.set_single_line_mode(true);
    lab.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    header.set_title_widget(Some(lab.as_ref()));
    header.set_show_title(true);
    Some(Rc::clone(&lab))
}
