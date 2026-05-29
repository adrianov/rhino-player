/// Sound (volume + audio tracks) and subtitle (style + tracks) header popovers.
struct HeaderPopovers {
    vol_adj: gtk::Adjustment,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_mute_btn: gtk::ToggleButton,
    audio_tracks_block: Rc<Cell<bool>>,
    audio_tracks_box: gtk::Box,
    audio_tracks_section: gtk::Box,
    vol_pop: gtk::Popover,
    vol_menu: gtk::MenuButton,
    sub_tracks_block: Rc<Cell<bool>>,
    sub_tracks_box: gtk::Box,
    sub_tracks_section: gtk::Box,
    sub_scale_adj: gtk::Adjustment,
    sub_color_btn: gtk::ColorDialogButton,
    sub_color_row: gtk::Box,
    sub_pop: gtk::Popover,
    sub_menu: gtk::MenuButton,
    sub_readout: gtk::Label,
}

fn build_header_popovers(sub_pref: &Rc<RefCell<db::SubPrefs>>) -> HeaderPopovers {
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
        .min_content_width(crate::header_menu_scroll::AUDIO_MIN_W)
        .max_content_height(crate::header_menu_scroll::AUDIO_MAX_H)
        .child(&audio_tracks_box)
        .build();
    audio_tracks_scrl.add_css_class(crate::header_menu_scroll::SCROLL_CLASS_AUDIO);
    #[cfg(target_os = "macos")]
    audio_tracks_scrl.set_min_content_width(280);
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
    #[cfg(target_os = "macos")]
    {
        vol_pop.set_has_arrow(false);
        crate::macos_header_menu::wire_popover(&vol_pop);
    }

    let vol_header_img = gtk::Image::from_icon_name("audio-volume-high-symbolic");
    vol_header_img.set_valign(gtk::Align::Center);
    let vol_readout = gtk::Label::new(Some("100%"));
    vol_readout.add_css_class("rp-vol-readout");
    vol_readout.set_valign(gtk::Align::Center);
    vol_readout.set_xalign(0.0);
    let vol_face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    vol_face.add_css_class("rp-vol-face");
    vol_face.set_valign(gtk::Align::Center);
    vol_face.append(&vol_header_img);
    vol_face.append(&vol_readout);
    let vol_menu = gtk::MenuButton::new();
    vol_menu.set_child(Some(&vol_face));
    vol_menu.set_tooltip_text(Some("Audio"));
    vol_menu.set_popover(Some(&vol_pop));
    vol_menu.add_css_class("flat");
    vol_menu.add_css_class("rp-vol-mbtn");
    vol_menu.set_hexpand(false);
    vol_menu.set_valign(gtk::Align::Center);
    vol_menu.set_always_show_arrow(false);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu::wire_menu_btn_open_guard(&vol_menu);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("audio", &vol_menu, &vol_pop);

    let sp_init = sub_pref.borrow().clone();
    let sub_tracks_block = Rc::new(Cell::new(false));
    let sub_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    sub_tracks_box.set_margin_top(2);
    let sub_tracks_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(crate::header_menu_scroll::SUB_MIN_W)
        .max_content_height(crate::header_menu_scroll::SUB_MAX_H)
        .child(&sub_tracks_box)
        .build();
    sub_tracks_scrl.add_css_class(crate::header_menu_scroll::SCROLL_CLASS_SUB);
    #[cfg(target_os = "macos")]
    sub_tracks_scrl.set_min_content_width(280);
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
    let sub_color_label = gtk::Label::new(Some("Text Color"));
    sub_color_label.set_xalign(0.0);
    sub_color_label.add_css_class("caption");
    let sub_color_row = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sub_color_row.append(&sub_color_label);
    sub_color_row.append(&sub_color_btn);
    sub_opts.append(&sub_color_row);

    let sub_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    sub_col.add_css_class("rp-popover-box");
    sub_col.append(&sub_tracks_section);
    sub_col.append(&sub_opts);

    let sub_pop = gtk::Popover::new();
    sub_pop.add_css_class("rp-header-popover");
    sub_pop.set_child(Some(&sub_col));
    header_popover_non_modal(&sub_pop);
    #[cfg(target_os = "macos")]
    {
        sub_pop.set_has_arrow(false);
        crate::macos_header_menu::wire_popover(&sub_pop);
    }

    let sub_header_img = gtk::Image::from_icon_name("media-view-subtitles-symbolic");
    sub_header_img.set_valign(gtk::Align::Center);
    let sub_readout = gtk::Label::new(Some("Off"));
    sub_readout.add_css_class("rp-sub-readout");
    sub_readout.set_valign(gtk::Align::Center);
    sub_readout.set_xalign(0.0);
    let sub_face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    sub_face.add_css_class("rp-sub-face");
    sub_face.set_valign(gtk::Align::Center);
    sub_face.append(&sub_header_img);
    sub_face.append(&sub_readout);

    let sub_menu = gtk::MenuButton::new();
    sub_menu.set_child(Some(&sub_face));
    sub_menu.set_tooltip_text(Some("Subtitles: tracks and style"));
    sub_menu.set_popover(Some(&sub_pop));
    sub_menu.add_css_class("flat");
    sub_menu.add_css_class("rp-sub-mbtn");
    sub_menu.set_hexpand(false);
    sub_menu.set_valign(gtk::Align::Center);
    sub_menu.set_always_show_arrow(false);
    sub_menu.set_visible(false);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu::wire_menu_btn_open_guard(&sub_menu);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("subtitles", &sub_menu, &sub_pop);

    HeaderPopovers {
        vol_adj,
        vol_header_img,
        vol_readout,
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
        sub_color_row,
        sub_pop,
        sub_menu,
        sub_readout,
    }
}
