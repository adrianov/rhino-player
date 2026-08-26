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
