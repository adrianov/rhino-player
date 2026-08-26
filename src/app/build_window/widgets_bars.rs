fn build_bottom_bar(
    wrap_prev: &gtk::Box,
    play_pause: &gtk::Button,
    wrap_next: &gtk::Box,
    time_left: &gtk::Label,
    seek: &gtk::Scale,
    time_right: &gtk::Label,
) -> (gtk::Box, gtk::Button) {
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
    let close_btn = close_video_bar_button();
    bottom.append(&close_btn);
    (bottom, close_btn)
}

fn new_recent_scroll_area() -> (
    gtk::Box,
    gtk::Box,
    [gtk::Box; 2],
    crate::recent_view::UndoBar,
    crate::recent_view::NoticeToast,
) {
    let (recent_scrl, flow_recent, recent_spacers, undo_bar, notice_toast) =
        recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    (
        recent_scrl,
        flow_recent,
        recent_spacers,
        undo_bar,
        notice_toast,
    )
}

fn window_menu_button(pref_menu: &gio::Menu) -> gtk::MenuButton {
    #[cfg(target_os = "macos")]
    let _ = pref_menu;
    #[cfg(not(target_os = "macos"))]
    {
        build_linux_main_menu_button(pref_menu)
    }
    #[cfg(target_os = "macos")]
    {
        gtk::MenuButton::new()
    }
}

fn mount_video_overlay(gl_area: &gtk::GLArea, recent_scrl: &gtk::Box) -> gtk::WindowHandle {
    let ovl = build_video_overlay(gl_area);
    let video_handle = gtk::WindowHandle::new();
    video_handle.set_child(Some(&ovl));
    ovl.add_overlay(recent_scrl);
    video_handle
}

fn close_video_bar_button() -> gtk::Button {
    let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
    close_btn.add_css_class("flat");
    close_btn.set_valign(gtk::Align::Center);
    close_btn.set_action_name(Some("app.close-video"));
    close_btn.set_margin_start(2);
    close_btn
}

fn build_video_overlay(child: &gtk::GLArea) -> gtk::Overlay {
    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(child));
    ovl
}

/// Menubar model (kept alive only on macOS) plus the preferences submenu.
struct AppMenus {
    pref_menu: gio::Menu,
    #[cfg(target_os = "macos")]
    menubar_model: gio::Menu,
}

impl AppMenus {
    fn build() -> Self {
        let (discard_menu_placeholder, pref_menu, menubar_model) = build_app_menus();
        drop(discard_menu_placeholder);
        #[cfg(not(target_os = "macos"))]
        drop(menubar_model);
        Self {
            pref_menu,
            #[cfg(target_os = "macos")]
            menubar_model,
        }
    }
}

/// Header-end buttons that need the window or recent area: main menu, fill, blackout.
struct HeaderButtons {
    menu_btn: gtk::MenuButton,
    fill_btn: gtk::Button,
    blackout_menu: gtk::Button,
    blackout_sync: Rc<crate::screen_blackout::BlackoutSync>,
}

impl HeaderButtons {
    fn build(
        win: &adw::ApplicationWindow,
        player: &Rc<RefCell<Option<MpvBundle>>>,
        pref_menu: &gio::Menu,
        recent_scrl: &gtk::Box,
    ) -> Self {
        let menu_btn = window_menu_button(pref_menu);
        let (fill_btn, _) = crate::video_fill::build_fill_header(win, player);
        let (blackout_menu, blackout_sync) =
            crate::screen_blackout::build_blackout_header(win, player, recent_scrl);
        Self {
            menu_btn,
            fill_btn,
            blackout_menu,
            blackout_sync,
        }
    }
}

/// Widget clusters built independently before the window shell is assembled.
struct WidgetGroups {
    win: adw::ApplicationWindow,
    outer_ovl: gtk::Overlay,
    chrome: PlaybackChromeRow,
    pops: HeaderPopovers,
    gl_area: gtk::GLArea,
    speed: SpeedMenuResult,
    smooth: SmoothToolbarWidgets,
    recent_scrl: gtk::Box,
    flow_recent: gtk::Box,
    recent_spacers: [gtk::Box; 2],
    undo_bar: crate::recent_view::UndoBar,
    notice_toast: crate::recent_view::NoticeToast,
}

impl WidgetGroups {
    fn build(
        app: &adw::Application,
        player: &Rc<RefCell<Option<MpvBundle>>>,
        video_pref: &Rc<RefCell<db::VideoPrefs>>,
        sub_pref: &Rc<RefCell<db::SubPrefs>>,
    ) -> Self {
        let win = build_main_application_window(app);
        let outer_ovl = gtk::Overlay::new();
        let chrome = build_playback_chrome_row();
        let pops = build_header_popovers(sub_pref);
        let gl_area = build_gl_video_area();
        let speed = build_speed_menu(player, &gl_area, video_pref, app);
        let smooth = build_smooth_video_toolbar();
        let (recent_scrl, flow_recent, recent_spacers, undo_bar, notice_toast) =
            new_recent_scroll_area();
        Self {
            win,
            outer_ovl,
            chrome,
            pops,
            gl_area,
            speed,
            smooth,
            recent_scrl,
            flow_recent,
            recent_spacers,
            undo_bar,
            notice_toast,
        }
    }
}
