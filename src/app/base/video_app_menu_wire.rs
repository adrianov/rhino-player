/// [`gio::Menu`] refs and header widgets threaded into [`register_video_app_actions`].
struct VideoAppMenuWire {
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    smooth_toolbar_status: Option<gtk::Label>,
}

fn stamp_smooth_toolbar_readout(lab: Option<&gtk::Label>, player: &Rc<RefCell<Option<MpvBundle>>>) {
    let Some(l) = lab else {
        return;
    };
    let text = if let Ok(g) = player.try_borrow() {
        g.as_ref()
            .map(|b| crate::video_pref::smooth_toolbar_fps_label(&b.mpv))
            .unwrap_or_else(|| "—".to_string())
    } else {
        return;
    };
    l.set_label(&text);
}
