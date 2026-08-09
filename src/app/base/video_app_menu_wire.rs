/// [`gio::Menu`] refs and header widgets threaded into [`register_video_app_actions`].
struct VideoAppMenuWire {
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    smooth_toolbar_btn: Option<gtk::Button>,
    smooth_toolbar_status: Option<gtk::Label>,
}

fn stamp_smooth_toolbar_readout(
    lab: Option<&gtk::Label>,
    btn: Option<&gtk::Button>,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    let Ok(g) = player.try_borrow() else {
        return;
    };
    let (fps_text, src_fps) = if let Some(b) = g.as_ref() {
        (
            crate::video_pref::smooth_toolbar_fps_label(&b.mpv),
            crate::video_pref::source_fps_label(&b.mpv),
        )
    } else {
        ("—".to_string(), None)
    };
    drop(g);
    if let Some(l) = lab {
        l.set_label(&fps_text);
    }
    if let Some(btn) = btn {
        sync_smooth_load_hold_face(btn);
        let tip = if crate::video_pref::smooth_load_hold_active() {
            crate::video_pref::smooth_load_hold_tooltip().to_string()
        } else {
            match src_fps {
                Some(src) => format!("Smooth Video ({src} → 60 FPS)"),
                None => SMOOTH60_MENU_LABEL.to_string(),
            }
        };
        if btn.tooltip_text().as_deref() != Some(&tip) {
            btn.set_tooltip_text(Some(&tip));
        }
    }
}

/// Small yellow warning beside the camera icon while Smooth is paused for external load.
fn sync_smooth_load_hold_face(btn: &gtk::Button) {
    let held = crate::video_pref::smooth_load_hold_active();
    let Some(face) = btn.child().and_then(|c| c.downcast::<gtk::Box>().ok()) else {
        return;
    };
    let warn = smooth_load_warn_image(&face);
    if warn.is_visible() != held {
        warn.set_visible(held);
    }
}

fn smooth_load_warn_image(face: &gtk::Box) -> gtk::Image {
    let mut child = face.first_child();
    while let Some(w) = child {
        let next = w.next_sibling();
        if w.has_css_class("rp-smooth-load-warn") {
            if let Ok(img) = w.downcast::<gtk::Image>() {
                return img;
            }
        }
        child = next;
    }
    let img = gtk::Image::from_icon_name("dialog-warning-symbolic");
    img.add_css_class("rp-smooth-load-warn");
    img.set_pixel_size(12);
    img.set_valign(gtk::Align::Center);
    img.set_visible(false);
    // Sit after the camera icon, before the FPS readout.
    if let Some(cam) = face.first_child() {
        face.insert_child_after(&img, Some(&cam));
    } else {
        face.append(&img);
    }
    img
}

fn sync_smooth_toolbar_on(btn: Option<&gtk::Button>, on: bool) {
    let Some(b) = btn else {
        return;
    };
    if on {
        b.add_css_class("rp-smooth-on");
    } else {
        crate::video_pref::smooth_load_hold_clear();
        b.remove_css_class("rp-smooth-on");
        sync_smooth_load_hold_face(b);
    }
}
