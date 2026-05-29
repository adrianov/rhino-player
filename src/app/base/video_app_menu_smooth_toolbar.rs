fn smooth_60_action(app: &adw::Application) -> Option<gio::SimpleAction> {
    app.lookup_action("smooth-60")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
}

/// Pref is on but the vapoursynth graph is missing or stale — toolbar click should re-attach, not turn off.
fn smooth_toolbar_needs_reattach(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
) -> bool {
    let vp = video_pref.borrow();
    if !vp.smooth_60 {
        return false;
    }
    let Ok(g) = player.try_borrow() else {
        return false;
    };
    let Some(b) = g.as_ref() else {
        return false;
    };
    if !video_pref::mpv_has_open_media(&b.mpv) {
        return false;
    };
    if !video_pref::smooth_wants_vapoursynth_vf(&b.mpv, Some(b), None) {
        return false;
    };
    !video_pref::vf_chain_has_vapoursynth(&b.mpv)
        || !video_pref::vf_smooth_matches_prefs(&b.mpv, &vp, Some(b))
}

fn schedule_smooth_toggle_reattach(
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    gl: gtk::GLArea,
) {
    let _ = glib::idle_add_local_once(move || {
        if let Some(b) = player.borrow().as_ref() {
            let mut vp = video_pref.borrow_mut();
            let _ = video_pref::reapply_60_if_still_missing(b, &mut vp);
        }
        gl.queue_render();
    });
}

fn wire_smooth_toolbar_button(
    app: &adw::Application,
    btn: &gtk::Button,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    gl: &gtk::GLArea,
    status: Option<&gtk::Label>,
) {
    sync_smooth_toolbar_on(Some(btn), video_pref.borrow().smooth_60);
    let app = app.clone();
    let player = Rc::clone(player);
    let video_pref = Rc::clone(video_pref);
    let gl = gl.clone();
    let status = status.cloned();
    let tip_btn = btn.clone();
    btn.connect_clicked(move |_| {
        let Some(a) = smooth_60_action(&app) else {
            return;
        };
        let cur = a.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        if cur && smooth_toolbar_needs_reattach(&player, &video_pref) {
            if let Some(plr) = player.borrow().as_ref() {
                let mut g = video_pref.borrow_mut();
                let r = video_pref::apply_mpv_video(plr, &mut g, None);
                if !r.smooth_auto_off {
                    schedule_smooth_toggle_reattach(
                        Rc::clone(&player),
                        Rc::clone(&video_pref),
                        gl.clone(),
                    );
                }
            }
            gl.queue_render();
            stamp_smooth_toolbar_readout(status.as_ref(), Some(&tip_btn), &player);
            return;
        }
        a.change_state(&(!cur).to_variant());
    });
}

fn sync_smooth_60_to_off(app: &adw::Application) {
    if let Some(a) = app.lookup_action("smooth-60") {
        a.change_state(&false.to_variant());
    }
}

/// Applies or reports an error after the user chose a VapourSynth path.
fn apply_vs_path_chosen(
    pl: &Rc<RefCell<Option<MpvBundle>>>,
    p: &Rc<RefCell<db::VideoPrefs>>,
    app: &adw::Application,
    smooth_toolbar_status: Option<&gtk::Label>,
    smooth_toolbar_btn: Option<&gtk::Button>,
) {
    if let Some(plr) = pl.borrow().as_ref() {
        let r = video_pref::apply_mpv_video(plr, &mut p.borrow_mut(), None);
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
