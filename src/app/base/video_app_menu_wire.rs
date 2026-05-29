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
    if let Some(l) = lab {
        l.set_label(&fps_text);
    }
    if let Some(btn) = btn {
        let tip = match src_fps {
            Some(src) => format!("Smooth Video ({src} → 60 FPS)"),
            None => SMOOTH60_MENU_LABEL.to_string(),
        };
        if btn.tooltip_text().as_deref() != Some(&tip) {
            btn.set_tooltip_text(Some(&tip));
        }
    }
}

fn sync_smooth_toolbar_on(btn: Option<&gtk::Button>, on: bool) {
    let Some(b) = btn else {
        return;
    };
    if on {
        b.add_css_class("rp-smooth-on");
    } else {
        b.remove_css_class("rp-smooth-on");
    }
}
