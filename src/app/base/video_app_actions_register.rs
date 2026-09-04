/// Registers the `app.*` video-preference actions. Each action gets its own registration helper;
/// this function only wires them in menu order and rebuilds the submenu from the initial prefs.
fn register_video_app_actions(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    gl_area: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    menu: VideoAppMenuWire,
) {
    let VideoAppMenuWire {
        pref_menu,
        seek_bar_on,
        smooth_toolbar_btn,
        smooth_toolbar_status,
    } = menu;
    let v0 = video_pref.borrow().clone();
    register_smooth_60_action(
        app,
        gl_area,
        player,
        &video_pref,
        v0.smooth_60,
        smooth_toolbar_btn.clone(),
        smooth_toolbar_status.clone(),
    );
    register_seek_bar_preview_action(app, &seek_bar_on);
    register_vs_custom_action(app, player, gl_area, &video_pref, &pref_menu);
    register_choose_vs_action(
        VsCustomCtx {
            app: app.clone(),
            pref_rc: Rc::clone(&video_pref),
            player: Rc::clone(player),
            gl_area: gl_area.clone(),
            pref_menu: pref_menu.clone(),
        },
        win,
        smooth_toolbar_status,
        smooth_toolbar_btn,
    );
    video_pref_submenu_rebuild(&pref_menu, &v0, app);
}

/// Build the stateful [smooth-60] action and bind its change-state handler context.
fn build_smooth_60_action(
    app: &adw::Application,
    gl_area: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    initial_on: bool,
    smooth_toolbar_btn: &Option<gtk::Button>,
    smooth_toolbar_status: &Option<gtk::Label>,
) -> gio::SimpleAction {
    let smooth_60 = gio::SimpleAction::new_stateful("smooth-60", None, &initial_on.to_variant());
    let smooth_ctx = Smooth60ToggleCtx {
        app: app.clone(),
        video_pref: Rc::clone(video_pref),
        player: Rc::clone(player),
        gl_area: gl_area.clone(),
        smooth_lbl: smooth_toolbar_status.clone(),
        smooth_btn: smooth_toolbar_btn.clone(),
    };
    smooth_60.connect_change_state(move |a, s| {
        let Some(s) = s else {
            return;
        };
        on_smooth_60_change_state(a, s, &smooth_ctx);
    });
    smooth_60
}

fn register_smooth_60_action(
    app: &adw::Application,
    gl_area: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    initial_on: bool,
    smooth_toolbar_btn: Option<gtk::Button>,
    smooth_toolbar_status: Option<gtk::Label>,
) {
    app.add_action(&build_smooth_60_action(
        app,
        gl_area,
        player,
        video_pref,
        initial_on,
        &smooth_toolbar_btn,
        &smooth_toolbar_status,
    ));
    if let Some(btn) = &smooth_toolbar_btn {
        wire_smooth_toolbar_button(
            app,
            btn,
            player,
            video_pref,
            gl_area,
            smooth_toolbar_status.as_ref(),
        );
    }
    stamp_smooth_toolbar_readout(
        smooth_toolbar_status.as_ref(),
        smooth_toolbar_btn.as_ref(),
        player,
    );
}

fn register_seek_bar_preview_action(app: &adw::Application, seek_bar_on: &Rc<Cell<bool>>) {
    let seek_bar_preview =
        gio::SimpleAction::new_stateful("seek-bar-preview", None, &seek_bar_on.get().to_variant());
    {
        let on = Rc::clone(seek_bar_on);
        seek_bar_preview.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(b) = s.get::<bool>() else {
                return;
            };
            a.set_state(s);
            on.set(b);
            db::save_seek_bar_preview(b);
            crate::user_action_log::act(format!(
                "preferences seek-bar-preview -> {}",
                if b { "on" } else { "off" }
            ));
        });
    }
    app.add_action(&seek_bar_preview);
}

fn register_vs_custom_action(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl_area: &gtk::GLArea,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    pref_menu: &gio::Menu,
) {
    let vs_custom = gio::SimpleAction::new_stateful(
        "vs-custom",
        None,
        &(!video_pref.borrow().vs_path.trim().is_empty()).to_variant(),
    );
    let ctx = Rc::new(VsCustomCtx {
        app: app.clone(),
        pref_rc: Rc::clone(video_pref),
        player: Rc::clone(player),
        gl_area: gl_area.clone(),
        pref_menu: pref_menu.clone(),
    });
    vs_custom.connect_change_state(move |a, s| {
        let Some(s) = s else {
            return;
        };
        on_vs_custom_change_state(a, s, &ctx);
    });
    app.add_action(&vs_custom);
}

include!("video_vs_choose_action.rs");
