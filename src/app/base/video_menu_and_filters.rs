#[cfg_attr(target_os = "macos", allow(dead_code))]
fn video_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("Video Files"));
    {
        f.add_mime_type("video/*");
        for s in video_ext::SUFFIX {
            f.add_suffix(s);
        }
        f.add_suffix("bdmv");
        f.add_suffix("bdm");
    }
    #[cfg(target_os = "macos")]
    {
        for s in video_ext::SUFFIX {
            f.add_pattern(&format!("*.{s}"));
            let up = s.to_uppercase();
            if up.as_str() != *s {
                f.add_pattern(&format!("*.{up}"));
            }
        }
        f.add_pattern("*.bdmv");
        f.add_pattern("*.BDMV");
        f.add_pattern("*.bdm");
        f.add_pattern("*.BDM");
    }
    f
}

fn vpy_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("VapourSynth Scripts"));
    f.add_suffix("vpy");
    #[cfg(target_os = "macos")]
    {
        f.add_pattern("*.vpy");
        f.add_pattern("*.VPY");
    }
    f
}

include!("toolbar_reveal_set.rs");

/// Rebuilds the **Preferences** submenu: Smooth 60, seek preview, optional `basename` for `video_vs_path`
/// ([vs-custom]), [choose-vs].
fn video_pref_submenu_rebuild(m: &gio::Menu, p: &db::VideoPrefs, app: &adw::Application) {
    m.remove_all();
    menu_append_action_icon(m, Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"), Some("camera-video-symbolic"));
    menu_append_action_icon(
        m,
        Some(SEEK_BAR_MENU_LABEL),
        Some("app.seek-bar-preview"),
        Some("sidebar-show-symbolic"),
    );
    if !p.vs_path.trim().is_empty() {
        let name = std::path::Path::new(p.vs_path.trim())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("script.vpy");
        menu_append_action_icon(m, Some(name), Some("app.vs-custom"), Some("text-x-generic-symbolic"));
    }
    menu_append_action_icon(
        m,
        Some("Choose VapourSynth Script (.vpy)…"),
        Some("app.choose-vs"),
        Some("document-properties-symbolic"),
    );
    if let Some(a) = app
        .lookup_action("vs-custom")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
    {
        a.set_state(&(!p.vs_path.trim().is_empty()).to_variant());
    }
}

/// Main menu: [db::VideoPrefs] and `app.*` actions for `gio::Menu` (before [win::present]).
fn handle_smooth_apply_result(
    app: &adw::Application,
    pl: &Rc<RefCell<Option<MpvBundle>>>,
    r: video_pref::MpvVideoApply,
) {
    if !r.smooth_auto_off {
        return;
    }
    let vf_still_on = pl
        .borrow()
        .as_ref()
        .is_some_and(|b| crate::video_pref::vf_chain_has_vapoursynth(&b.mpv));
    if vf_still_on {
        eprintln!(
            "[rhino] video: vf add error ignored — vapoursynth still active; keeping Smooth on"
        );
        return;
    }
    sync_smooth_60_to_off(app);
    show_smooth_setup_dialog(app);
}

struct Smooth60ToggleCtx {
    app: adw::Application,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl_area: gtk::GLArea,
    smooth_lbl: Option<gtk::Label>,
    smooth_btn: Option<gtk::Button>,
}

fn on_smooth_60_change_state(action: &gio::SimpleAction, value: &glib::Variant, ctx: &Smooth60ToggleCtx) {
    let Some(b) = value.get::<bool>() else {
        return;
    };
    if smooth_60_action_programmatic() {
        crate::user_action_log::act(format!("smooth-60 menu sync (programmatic) -> {b}"));
        action.set_state(value);
        sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), b);
        return;
    }
    crate::user_action_log::act(format!(
        "smooth-60 menu -> {}",
        if b { "on" } else { "off" }
    ));
    if b {
        crate::app::cancel_smooth_60_transport_resync();
        crate::video_pref::cancel_deferred_vf_swap();
    }
    if b && !can_find_mvtools(&ctx.video_pref.borrow()) {
        let mut g = ctx.video_pref.borrow_mut();
        g.smooth_60 = false;
        db::save_video(&g);
        action.set_state(&false.to_variant());
        sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), false);
        show_smooth_setup_dialog(&ctx.app);
        ctx.gl_area.queue_render();
        stamp_smooth_toolbar_readout(ctx.smooth_lbl.as_ref(), ctx.smooth_btn.as_ref(), &ctx.player);
        return;
    }
    action.set_state(value);
    sync_smooth_toolbar_on(ctx.smooth_btn.as_ref(), b);
    {
        let mut g = ctx.video_pref.borrow_mut();
        g.smooth_60 = b;
        db::save_video(&g);
    }
    if ctx.player.borrow().as_ref().is_some() {
        let mut g = ctx.video_pref.borrow_mut();
        let reload = b && video_pref::smooth_user_enable_playing_reset(&ctx.player, &mut g);
        if !reload {
            let r = video_pref::apply_mpv_video(&ctx.player, &mut g, None);
            handle_smooth_apply_result(&ctx.app, &ctx.player, r);
        }
    }
    ctx.gl_area.queue_render();
    stamp_smooth_toolbar_readout(ctx.smooth_lbl.as_ref(), ctx.smooth_btn.as_ref(), &ctx.player);
}

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
    let app_s = app.clone();
    let smooth_60 = gio::SimpleAction::new_stateful("smooth-60", None, &v0.smooth_60.to_variant());
    let smooth_ctx = Smooth60ToggleCtx {
        app: app_s,
        video_pref: Rc::clone(&video_pref),
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
    app.add_action(&smooth_60);
    if let Some(ref btn) = smooth_toolbar_btn {
        wire_smooth_toolbar_button(
            app,
            btn,
            player,
            &video_pref,
            gl_area,
            smooth_toolbar_status.as_ref(),
        );
    }
    stamp_smooth_toolbar_readout(
        smooth_toolbar_status.as_ref(),
        smooth_toolbar_btn.as_ref(),
        player,
    );

    let seek_bar_preview =
        gio::SimpleAction::new_stateful("seek-bar-preview", None, &seek_bar_on.get().to_variant());
    {
        let on = Rc::clone(&seek_bar_on);
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

    let vs_custom = gio::SimpleAction::new_stateful(
        "vs-custom",
        None,
        &(!v0.vs_path.trim().is_empty()).to_variant(),
    );
    {
        let p = Rc::clone(&video_pref);
        let pl = Rc::clone(player);
        let gla = gl_area.clone();
        let app_c = app.clone();
        let pref = pref_menu.clone();
        vs_custom.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(checked) = s.get::<bool>() else {
                return;
            };
            a.set_state(s);
            if checked {
                return;
            }
            crate::user_action_log::act("preferences vs-custom -> off (bundled script)");
            {
                let mut g = p.borrow_mut();
                if g.vs_path.trim().is_empty() {
                    return;
                }
                g.vs_path.clear();
                db::save_video(&g);
            }
            if pl.borrow().as_ref().is_some() {
                let r = {
                    let mut g = p.borrow_mut();
                    video_pref::apply_mpv_video(&pl, &mut g, None)
                };
                if r.smooth_auto_off {
                    sync_smooth_60_to_off(&app_c);
                    show_smooth_setup_dialog(&app_c);
                }
            }
            video_pref_submenu_rebuild(&pref, &p.borrow(), &app_c);
            gla.queue_render();
        });
    }
    app.add_action(&vs_custom);

    let choose = gio::SimpleAction::new("choose-vs", None);
    {
        let app2 = app.clone();
        let w = win.clone();
        let p = Rc::clone(&video_pref);
        let pl = Rc::clone(player);
        let gla = gl_area.clone();
        let pref = pref_menu.clone();
        let smooth_pick_lbl = smooth_toolbar_status.clone();
        let smooth_pick_btn = smooth_toolbar_btn.clone();
        choose.connect_activate(move |_, _| {
            crate::user_action_log::act("menu choose VapourSynth script");
            let vf = vpy_file_filter();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&vf);
            let dialog = gtk::FileDialog::builder()
                .title("VapourSynth Script")
                .modal(true)
                .filters(&filters)
                .default_filter(&vf)
                .build();
            let app3 = app2.clone();
            let p2 = p.clone();
            let pl2 = Rc::clone(&pl);
            let gl2 = gla.clone();
            let pref2 = pref.clone();
            let smooth_pick_lbl = smooth_pick_lbl.clone();
            let smooth_pick_btn = smooth_pick_btn.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else {
                    return;
                };
                let Some(path) = file.path() else {
                    eprintln!("[rhino] choose-vs: path required");
                    return;
                };
                if !can_find_mvtools(&p2.borrow()) {
                    p2.borrow_mut().smooth_60 = false;
                    db::save_video(&p2.borrow());
                    sync_smooth_60_to_off(&app3);
                    show_smooth_setup_dialog(&app3);
                    stamp_smooth_toolbar_readout(
                        smooth_pick_lbl.as_ref(),
                        smooth_pick_btn.as_ref(),
                        &pl2,
                    );
                    return;
                }
                {
                    let mut g = p2.borrow_mut();
                    g.vs_path = path.to_str().unwrap_or("").to_string();
                    g.smooth_60 = true;
                    db::save_video(&g);
                }
                apply_vs_path_chosen(
                    &pl2,
                    &p2,
                    &app3,
                    smooth_pick_lbl.as_ref(),
                    smooth_pick_btn.as_ref(),
                );
                video_pref_submenu_rebuild(&pref2, &p2.borrow(), &app3);
                gl2.queue_render();
            });
        });
    }
    app.add_action(&choose);
    video_pref_submenu_rebuild(&pref_menu, &v0, app);
}

