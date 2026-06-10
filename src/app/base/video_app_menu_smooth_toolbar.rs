thread_local! {
    /// [sync_smooth_60_to_off] / prefs sync: update the action without re-running **`apply_mpv_video`**.
    static SMOOTH_60_ACTION_PROGRAMMATIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn smooth_60_action_programmatic() -> bool {
    SMOOTH_60_ACTION_PROGRAMMATIC.get()
}

fn smooth_60_action(app: &adw::Application) -> Option<gio::SimpleAction> {
    app.lookup_action("smooth-60")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
}


fn wire_smooth_toolbar_button(
    app: &adw::Application,
    btn: &gtk::Button,
    _player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    _gl: &gtk::GLArea,
    _status: Option<&gtk::Label>,
) {
    sync_smooth_toolbar_on(Some(btn), video_pref.borrow().smooth_60);
    let app = app.clone();
    btn.connect_clicked(move |_| {
        let Some(a) = smooth_60_action(&app) else {
            return;
        };
        let cur = a.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        crate::user_action_log::act(format!(
            "smooth-60 toolbar button -> {}",
            if cur { "off" } else { "on" }
        ));
        a.change_state(&(!cur).to_variant());
    });
}

fn sync_smooth_60_to_off(app: &adw::Application) {
    let Some(a) = smooth_60_action(app) else {
        return;
    };
    if a.state().and_then(|v| v.get::<bool>()) != Some(true) {
        return;
    }
    SMOOTH_60_ACTION_PROGRAMMATIC.set(true);
    a.set_state(&false.to_variant());
    SMOOTH_60_ACTION_PROGRAMMATIC.set(false);
}

/// Applies or reports an error after the user chose a VapourSynth path.
fn apply_vs_path_chosen(
    pl: &Rc<RefCell<Option<MpvBundle>>>,
    p: &Rc<RefCell<db::VideoPrefs>>,
    app: &adw::Application,
    smooth_toolbar_status: Option<&gtk::Label>,
    smooth_toolbar_btn: Option<&gtk::Button>,
) {
    if pl.borrow().as_ref().is_some() {
        let r = video_pref::apply_mpv_video(pl, &mut p.borrow_mut(), None);
        if r.smooth_auto_off {
            sync_smooth_60_to_off(app);
            show_smooth_setup_dialog(app);
        } else if let Some(sa) = smooth_60_action(app) {
            sa.set_state(&p.borrow().smooth_60.to_variant());
        }
        stamp_smooth_toolbar_readout(smooth_toolbar_status, smooth_toolbar_btn, pl);
    } else if let Some(sa) = smooth_60_action(app) {
        sa.set_state(&true.to_variant());
        stamp_smooth_toolbar_readout(smooth_toolbar_status, smooth_toolbar_btn, pl);
    }
}
