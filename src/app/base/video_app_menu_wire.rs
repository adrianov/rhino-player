/// [`gio::Menu`] refs and header widgets threaded into [`register_video_app_actions`].
struct VideoAppMenuWire {
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    smooth_toolbar_status: Option<gtk::Label>,
}

fn stamp_smooth_toolbar_status(lab: Option<&gtk::Label>, on: bool) {
    let Some(l) = lab else {
        return;
    };
    l.set_label(if on { "On" } else { "Off" });
}

fn sync_smooth_toolbar_from_action(app: &adw::Application, lab: Option<&gtk::Label>) {
    let Some(l) = lab else {
        return;
    };
    let on = app
        .lookup_action("smooth-60")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
        .and_then(|a| a.state())
        .and_then(|v| v.get::<bool>())
        .unwrap_or(false);
    stamp_smooth_toolbar_status(Some(l), on);
}
