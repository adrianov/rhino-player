/// Tracks list + size/color controls for the subtitle popover.
struct SubPopoverContent {
    tracks_block: Rc<Cell<bool>>,
    tracks_box: gtk::Box,
    tracks_section: gtk::Box,
    scale_adj: gtk::Adjustment,
    color_btn: gtk::ColorDialogButton,
    color_row: gtk::Box,
    opts: gtk::Box,
}

/// Subtitle side of [HeaderPopovers]: tracks section, size/color rows, popover, toolbar face.
struct SubPopoverParts {
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

fn build_subtitle_popover(sub_pref: &Rc<RefCell<db::SubPrefs>>) -> SubPopoverParts {
    let c = build_sub_content(sub_pref);
    let sub_pop = header_popover_column_shell(&c.tracks_section, &c.opts);
    let (sub_readout, sub_face) = build_sub_face();
    SubPopoverParts {
        sub_tracks_block: c.tracks_block,
        sub_tracks_box: c.tracks_box,
        sub_tracks_section: c.tracks_section,
        sub_scale_adj: c.scale_adj,
        sub_color_btn: c.color_btn,
        sub_color_row: c.color_row,
        sub_menu: build_sub_menu(&sub_face, &sub_pop),
        sub_pop,
        sub_readout,
    }
}

/// Track list plus the size / text-color control column.
fn build_sub_content(sub_pref: &Rc<RefCell<db::SubPrefs>>) -> SubPopoverContent {
    let tracks_block = Rc::new(Cell::new(false));
    let (tracks_box, tracks_section) = track_list_section(
        crate::header_menu_scroll::SUB_MIN_W,
        crate::header_menu_scroll::SUB_MAX_H,
        crate::header_menu_scroll::SCROLL_CLASS_SUB,
    );
    let sp_init = sub_pref.borrow().clone();
    let (scale_adj, scale) = build_sub_scale_row(sp_init.scale);
    let color_btn = build_sub_color_btn(sp_init.color);
    let (opts, color_row) = build_sub_style_rows(&scale, &color_btn);
    SubPopoverContent {
        tracks_block,
        tracks_box,
        tracks_section,
        scale_adj,
        color_btn,
        color_row,
        opts,
    }
}

/// Subtitle-size adjustment and scale (mpv `sub-scale` range).
fn build_sub_scale_row(scale_init: f64) -> (gtk::Adjustment, gtk::Scale) {
    let adj = gtk::Adjustment::new(scale_init, 0.3, 2.0, 0.05, 0.1, 0.0);
    let scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&adj));
    scale.set_draw_value(true);
    scale.set_digits(2);
    scale.set_hexpand(true);
    scale.set_size_request(240, -1);
    scale.set_tooltip_text(Some("Subtitle size (mpv sub-scale)"));
    (adj, scale)
}

fn build_sub_color_btn(color_init: u32) -> gtk::ColorDialogButton {
    let btn = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    btn.set_rgba(&sub_prefs::u32_to_rgba(color_init));
    btn.set_tooltip_text(Some("Subtitle text color"));
    btn
}

/// Size row plus text-color row stacked in one options column; returns `(opts, color_row)`.
fn build_sub_style_rows(
    sub_scale: &gtk::Scale,
    sub_color_btn: &gtk::ColorDialogButton,
) -> (gtk::Box, gtk::Box) {
    let opts = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let size_label = gtk::Label::new(Some("Size"));
    size_label.set_xalign(0.0);
    size_label.add_css_class("caption");
    opts.append(&size_label);
    opts.append(sub_scale);
    let color_label = gtk::Label::new(Some("Text Color"));
    color_label.set_xalign(0.0);
    color_label.add_css_class("caption");
    let color_row = gtk::Box::new(gtk::Orientation::Vertical, 0);
    color_row.append(&color_label);
    color_row.append(sub_color_btn);
    opts.append(&color_row);
    (opts, color_row)
}

/// Header icon + readout label inside one face box.
fn build_sub_face() -> (gtk::Label, gtk::Box) {
    let img = gtk::Image::from_icon_name("media-view-subtitles-symbolic");
    img.set_valign(gtk::Align::Center);
    let readout = gtk::Label::new(Some("Off"));
    readout.add_css_class("rp-sub-readout");
    readout.set_valign(gtk::Align::Center);
    readout.set_xalign(0.0);
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    face.add_css_class("rp-sub-face");
    face.set_valign(gtk::Align::Center);
    face.append(&img);
    face.append(&readout);
    (readout, face)
}

/// Toolbar menu button wrapping the subtitle face and popover; hidden until subtitles exist.
fn build_sub_menu(sub_face: &gtk::Box, sub_pop: &gtk::Popover) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.set_child(Some(sub_face));
    menu.set_tooltip_text(Some("Subtitles: tracks and style"));
    menu.set_popover(Some(sub_pop));
    menu.add_css_class("flat");
    menu.add_css_class("rp-sub-mbtn");
    menu.set_hexpand(false);
    menu.set_valign(gtk::Align::Center);
    menu.set_always_show_arrow(false);
    menu.set_visible(false);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu::wire_menu_btn_open_guard(&menu);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("subtitles", &menu, sub_pop);
    menu
}
