// Speed popover widget builders: list rows, popover shell, menu button, face, scroller.
fn build_speed_list() -> gtk::ListBox {
    let speed_list = gtk::ListBox::new();
    // Linux: row-activated on single click (GTK does not reliably apply speed via row-selected
    // when activate-on-single-click is false). macOS: row-selected + false avoids spurious apply
    // while the opening click settles (pick guard).
    #[cfg(not(target_os = "macos"))]
    speed_list.set_activate_on_single_click(true);
    #[cfg(target_os = "macos")]
    speed_list.set_activate_on_single_click(false);
    speed_list.add_css_class("rich-list");
    for s in &playback_speed::SPEEDS {
        speed_list.append(&build_speed_row(s));
    }
    speed_list
}

fn build_speed_row(step: &f64) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let lab = gtk::Label::new(Some(&playback_speed::format_step(*step)));
    lab.set_halign(gtk::Align::Start);
    lab.set_margin_start(10);
    lab.set_margin_end(10);
    lab.set_margin_top(6);
    lab.set_margin_bottom(6);
    row.set_child(Some(&lab));
    row
}

fn wrap_speed_list_in_popover(speed_list: &gtk::ListBox) -> gtk::Popover {
    let speed_scrl = build_speed_scroller(speed_list);

    let speed_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    speed_col.add_css_class("rp-popover-box");
    speed_col.append(&speed_scrl);

    let speed_pop = gtk::Popover::new();
    speed_pop.add_css_class("rp-header-popover");
    speed_pop.set_child(Some(&speed_col));
    header_popover_non_modal(&speed_pop);
    #[cfg(target_os = "macos")]
    {
        speed_pop.set_has_arrow(false);
        crate::macos_header_menu::wire_popover(&speed_pop);
    }
    speed_pop
}

fn build_speed_mbtn(speed_pop: &gtk::Popover) -> gtk::MenuButton {
    let speed_mbtn = gtk::MenuButton::new();
    speed_mbtn.set_popover(Some(speed_pop));
    speed_mbtn.set_tooltip_text(Some("Playback speed"));
    speed_mbtn.set_sensitive(false);
    speed_mbtn.add_css_class("flat");
    speed_mbtn.add_css_class("rp-speed-mbtn");
    speed_mbtn.set_hexpand(false);
    speed_mbtn.set_valign(gtk::Align::Center);
    speed_mbtn.set_always_show_arrow(false);
    speed_mbtn
}

/// Icon + rate caption share one hit target (horizontal row keeps header / fullscreen
/// toolbar row height unchanged).
fn build_speed_face(readout: &gtk::Label) -> gtk::Box {
    let speed_icon = gtk::Image::from_icon_name("speedometer-symbolic");
    speed_icon.set_valign(gtk::Align::Center);

    let speed_face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    speed_face.add_css_class("rp-speed-face");
    speed_face.set_valign(gtk::Align::Center);
    speed_face.append(&speed_icon);
    speed_face.append(readout);
    speed_face
}

fn build_speed_scroller(speed_list: &gtk::ListBox) -> gtk::ScrolledWindow {
    let speed_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .max_content_height(crate::header_menu_scroll::SPEED_MAX_H)
        .child(speed_list)
        .build();
    speed_scrl.add_css_class(crate::header_menu_scroll::SCROLL_CLASS_SPEED);
    speed_scrl
}
