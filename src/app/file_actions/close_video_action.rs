// Close-video action: activation policy + browse-visibility sync.

fn make_close_video_action(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_scrl: &gtk::Box,
    on_browse_back: &Rc<dyn Fn(bool)>,
) -> gio::SimpleAction {
    let close_video = gio::SimpleAction::new("close-video", None);
    {
        let p = player.clone();
        let r = recent_scrl.clone();
        let bb = on_browse_back.clone();
        let app_q = app.clone();
        close_video.connect_activate(move |_, _| {
            if r.is_visible() {
                crate::user_action_log::act("close video (browse) -> quit");
                app_q.activate_action("quit", None);
                return;
            }
            if !crate::app::has_loaded_local_media(&p) {
                crate::user_action_log::act("close video (no media loaded) -> quit");
                app_q.activate_action("quit", None);
                return;
            }
            crate::user_action_log::act("close video button -> back to browse");
            bb(true);
        });
    }
    close_video
}

/// Refresh enablement/tooltip whenever the browse grid visibility flips and once at idle.
fn wire_close_video_visible_sync(
    recent_scrl: &gtk::Box,
    close_video: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    tip: &gtk::Button,
) {
    {
        let cv = close_video.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        let tip = tip.clone();
        recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
            sync_close_video_action(&cv, &tip, &p, &r);
        });
    }
    let _ = glib::idle_add_local_once({
        let cv = close_video.clone();
        let p = player.clone();
        let r = recent_scrl.clone();
        let tip = tip.clone();
        move || sync_close_video_action(&cv, &tip, &p, &r)
    });
}
