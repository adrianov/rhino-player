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
        stamp_smooth_toolbar_readout(ctx.smooth_lbl.as_ref(), ctx.smooth_btn.as_ref(), &ctx.player);
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
        let reload = {
            let mut g = ctx.video_pref.borrow_mut();
            let reload = b && video_pref::smooth_user_enable_playing_reset(&ctx.player, &mut g);
            if !reload {
                let r = video_pref::apply_mpv_video(&ctx.player, &mut g, None);
                handle_smooth_apply_result(&ctx.app, &ctx.player, r);
            }
            reload
        };
        // Release pause-hold if a playing Smooth-on reload was aborted (e.g. toggle off mid-reload).
        if !reload {
            if let Some(bundle) = ctx.player.borrow().as_ref() {
                video_pref::maybe_unpause_after_smooth_reload(&bundle.mpv);
            }
        }
    }
    ctx.gl_area.queue_render();
    stamp_smooth_toolbar_readout(ctx.smooth_lbl.as_ref(), ctx.smooth_btn.as_ref(), &ctx.player);
}

include!("video_app_actions_register.rs");

