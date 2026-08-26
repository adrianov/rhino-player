struct SmoothToolbarWidgets {
    smooth_btn: gtk::Button,
    smooth_status: gtk::Label,
}

/// Header Smooth control: icon + badge for **playing** FPS; title/intent stay in the tooltip.
fn build_smooth_video_toolbar() -> SmoothToolbarWidgets {
    let smooth_btn = new_smooth_button();

    let smooth_status = gtk::Label::new(Some("—"));
    smooth_status.add_css_class("rp-smooth-readout");
    smooth_status.set_xalign(0.0);
    smooth_status.set_valign(gtk::Align::Center);

    smooth_btn.set_child(Some(&new_smooth_face(&smooth_status)));

    SmoothToolbarWidgets {
        smooth_btn,
        smooth_status,
    }
}

fn new_smooth_button() -> gtk::Button {
    let smooth_btn = gtk::Button::new();
    smooth_btn.add_css_class("flat");
    smooth_btn.add_css_class("rp-smooth-mbtn");
    smooth_btn.set_valign(gtk::Align::Center);
    smooth_btn.set_tooltip_text(Some(SMOOTH60_MENU_LABEL));
    smooth_btn.set_cursor_from_name(Some("pointer"));
    smooth_btn
}

fn new_smooth_face(status: &gtk::Label) -> gtk::Box {
    let img = gtk::Image::from_icon_name("camera-video-symbolic");
    img.set_valign(gtk::Align::Center);

    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    face.add_css_class("rp-smooth-face");
    face.set_valign(gtk::Align::Center);
    face.append(&img);
    face.append(status);
    face
}
