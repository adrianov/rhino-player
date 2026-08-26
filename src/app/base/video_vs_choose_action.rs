// VapourSynth script actions: `app.vs-custom` toggle, `app.choose-vs` chooser, and the completion
// path. Split out of video_app_actions_register.rs (include!'d into the same module scope).

/// State shared by the [vs-custom] change-state handler and its turn-off path.
struct VsCustomCtx {
    app: adw::Application,
    pref_rc: Rc<RefCell<db::VideoPrefs>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl_area: gtk::GLArea,
    pref_menu: gio::Menu,
}

/// Context threaded from the activating click through the async dialog callback.
struct VpyChooseDeps {
    app: adw::Application,
    win: adw::ApplicationWindow,
    pref_rc: Rc<RefCell<db::VideoPrefs>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl_area: gtk::GLArea,
    pref_menu: gio::Menu,
    smooth_lbl: Option<gtk::Label>,
    smooth_btn: Option<gtk::Button>,
}

fn open_vpy_dialog(deps: Rc<VpyChooseDeps>) {
    let vf = vpy_file_filter();
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    let win = deps.win.clone();
    let dialog = gtk::FileDialog::builder()
        .title("VapourSynth Script")
        .modal(true)
        .filters(&filters)
        .default_filter(&vf)
        .build();
    dialog.open(Some(&win), None::<&gio::Cancellable>, move |res| {
        vpy_dialog_done(&deps, res);
    });
}

fn vpy_dialog_done(deps: &VpyChooseDeps, res: Result<gio::File, glib::Error>) {
    let Ok(file) = res else {
        return;
    };
    let Some(path) = file.path() else {
        eprintln!("[rhino] choose-vs: path required");
        return;
    };
    if !can_find_mvtools(&deps.pref_rc.borrow()) {
        reject_vpy_missing_mvtools(deps);
        return;
    }
    apply_chosen_vpy(deps, &path);
}

/// mvtools not found: revert Smooth, persist, and surface the setup dialog instead of applying.
fn reject_vpy_missing_mvtools(deps: &VpyChooseDeps) {
    deps.pref_rc.borrow_mut().smooth_60 = false;
    db::save_video(&deps.pref_rc.borrow());
    sync_smooth_60_to_off(&deps.app);
    show_smooth_setup_dialog(&deps.app);
    stamp_smooth_toolbar_readout(
        deps.smooth_lbl.as_ref(),
        deps.smooth_btn.as_ref(),
        &deps.player,
    );
}

/// Persist the chosen script, enable Smooth, apply it to mpv, and resync the submenu/chrome.
fn apply_chosen_vpy(deps: &VpyChooseDeps, path: &Path) {
    {
        let mut g = deps.pref_rc.borrow_mut();
        g.vs_path = path.to_str().unwrap_or("").to_string();
        g.smooth_60 = true;
        db::save_video(&g);
    }
    apply_vs_path_chosen(
        &deps.player,
        &deps.pref_rc,
        &deps.app,
        deps.smooth_lbl.as_ref(),
        deps.smooth_btn.as_ref(),
    );
    video_pref_submenu_rebuild(&deps.pref_menu, &deps.pref_rc.borrow(), &deps.app);
    deps.gl_area.queue_render();
}

fn register_choose_vs_action(
    core: VsCustomCtx,
    win: &adw::ApplicationWindow,
    smooth_lbl: Option<gtk::Label>,
    smooth_btn: Option<gtk::Button>,
) {
    let choose = gio::SimpleAction::new("choose-vs", None);
    let deps = Rc::new(VpyChooseDeps {
        app: core.app.clone(),
        win: win.clone(),
        pref_rc: core.pref_rc,
        player: core.player,
        gl_area: core.gl_area,
        pref_menu: core.pref_menu,
        smooth_lbl,
        smooth_btn,
    });
    choose.connect_activate(move |_, _| {
        crate::user_action_log::act("menu choose VapourSynth script");
        open_vpy_dialog(Rc::clone(&deps));
    });
    core.app.add_action(&choose);
}

fn on_vs_custom_change_state(a: &gio::SimpleAction, s: &glib::Variant, ctx: &VsCustomCtx) {
    let Some(checked) = s.get::<bool>() else {
        return;
    };
    a.set_state(s);
    if !checked {
        vs_custom_turned_off(ctx);
    }
}

/// Unchecking [vs-custom]: drop the script path, re-apply mpv video, and resync chrome.
fn vs_custom_turned_off(ctx: &VsCustomCtx) {
    crate::user_action_log::act("preferences vs-custom -> off (bundled script)");
    {
        let mut g = ctx.pref_rc.borrow_mut();
        if g.vs_path.trim().is_empty() {
            return;
        }
        g.vs_path.clear();
        db::save_video(&g);
    }
    if ctx.player.borrow().as_ref().is_some() {
        let r = {
            let mut g = ctx.pref_rc.borrow_mut();
            video_pref::apply_mpv_video(&ctx.player, &mut g, None)
        };
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&ctx.app);
            show_smooth_setup_dialog(&ctx.app);
        }
    }
    video_pref_submenu_rebuild(&ctx.pref_menu, &ctx.pref_rc.borrow(), &ctx.app);
    ctx.gl_area.queue_render();
}
