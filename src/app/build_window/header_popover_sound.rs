/// Sound side of [HeaderPopovers]: volume row, audio-tracks section, popover, toolbar face.
struct SoundPopoverParts {
    vol_adj: gtk::Adjustment,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_mute_btn: gtk::ToggleButton,
    audio_tracks_block: Rc<Cell<bool>>,
    audio_tracks_box: gtk::Box,
    audio_tracks_section: gtk::Box,
    vol_pop: gtk::Popover,
    vol_menu: gtk::MenuButton,
}

fn build_sound_popover() -> SoundPopoverParts {
    let (vol_adj, vol_scale) = build_vol_scale();
    let (vol_mute_btn, vol_row) = build_vol_mute_and_row(&vol_scale);
    let audio_tracks_block = Rc::new(Cell::new(false));
    let (audio_tracks_box, audio_tracks_section) = track_list_section(
        crate::header_menu_scroll::AUDIO_MIN_W,
        crate::header_menu_scroll::AUDIO_MAX_H,
        crate::header_menu_scroll::SCROLL_CLASS_AUDIO,
    );
    let vol_pop = header_popover_column_shell(&vol_row, &audio_tracks_section);
    let (vol_header_img, vol_readout, vol_face) = build_vol_face();
    SoundPopoverParts {
        vol_adj,
        vol_header_img,
        vol_readout,
        vol_mute_btn,
        audio_tracks_block,
        audio_tracks_box,
        audio_tracks_section,
        vol_menu: build_vol_menu(&vol_face, &vol_pop),
        vol_pop,
    }
}

/// Volume adjustment plus its horizontal toolbar scale.
fn build_vol_scale() -> (gtk::Adjustment, gtk::Scale) {
    let vol_adj = gtk::Adjustment::new(100.0, 0.0, 100.0, 1.0, 5.0, 0.0);
    let vol_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&vol_adj));
    vol_scale.set_draw_value(false);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(240, -1);
    vol_scale.set_valign(gtk::Align::Center);
    vol_scale.set_tooltip_text(Some("Volume"));
    vol_scale.add_css_class("rp-vol");
    (vol_adj, vol_scale)
}

/// Mute toggle followed by the volume scale in one centered row.
fn build_vol_mute_and_row(vol_scale: &gtk::Scale) -> (gtk::ToggleButton, gtk::Box) {
    let mute_btn = gtk::ToggleButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .valign(gtk::Align::Center)
        .vexpand(false)
        .tooltip_text("Mute")
        .build();
    mute_btn.add_css_class("flat");
    mute_btn.add_css_class("circular");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&mute_btn);
    row.append(vol_scale);
    (mute_btn, row)
}

/// Scrollable track list shared by both popovers: list box plus hidden section wrapper.
fn track_list_section(min_w: i32, max_h: i32, scroll_class: &str) -> (gtk::Box, gtk::Box) {
    let tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    tracks_box.set_margin_top(2);
    tracks_box.add_css_class("rp-track-list-box");
    let scrl = track_list_scroller(&tracks_box, min_w, max_h, scroll_class);
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.append(&scrl);
    section.set_visible(false);
    (tracks_box, section)
}

fn track_list_scroller(
    child: &gtk::Box,
    min_w: i32,
    max_h: i32,
    scroll_class: &str,
) -> gtk::ScrolledWindow {
    let scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(min_w)
        .max_content_height(max_h)
        .child(child)
        .build();
    scrl.add_css_class(scroll_class);
    #[cfg(target_os = "macos")]
    scrl.set_min_content_width(280);
    scrl
}

/// Shared popover assembly: vertical `rp-popover-box` column inside a non-modal shell
/// (plus macOS arrow/media-key handling).
fn header_popover_column_shell(first: &gtk::Box, second: &gtk::Box) -> gtk::Popover {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    col.add_css_class("rp-popover-box");
    col.append(first);
    col.append(second);
    let pop = gtk::Popover::new();
    pop.add_css_class("rp-header-popover");
    pop.set_child(Some(&col));
    header_popover_non_modal(&pop);
    #[cfg(target_os = "macos")]
    {
        pop.set_has_arrow(false);
        crate::macos_header_menu::wire_popover(&pop);
    }
    pop
}

/// Header icon + readout label inside one face box.
fn build_vol_face() -> (gtk::Image, gtk::Label, gtk::Box) {
    let img = gtk::Image::from_icon_name("audio-volume-high-symbolic");
    img.set_valign(gtk::Align::Center);
    let readout = gtk::Label::new(Some("100%"));
    readout.add_css_class("rp-vol-readout");
    readout.set_valign(gtk::Align::Center);
    readout.set_xalign(0.0);
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    face.add_css_class("rp-vol-face");
    face.set_valign(gtk::Align::Center);
    face.append(&img);
    face.append(&readout);
    (img, readout, face)
}

/// Toolbar menu button wrapping the volume face and sound popover.
fn build_vol_menu(vol_face: &gtk::Box, vol_pop: &gtk::Popover) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.set_child(Some(vol_face));
    menu.set_tooltip_text(Some("Audio"));
    menu.set_popover(Some(vol_pop));
    menu.add_css_class("flat");
    menu.add_css_class("rp-vol-mbtn");
    menu.set_hexpand(false);
    menu.set_valign(gtk::Align::Center);
    menu.set_always_show_arrow(false);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu::wire_menu_btn_open_guard(&menu);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("audio", &menu, vol_pop);
    menu
}
