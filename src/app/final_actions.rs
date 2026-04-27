struct FinalActionCtx {
    app: adw::Application,
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    win_aspect: Rc<Cell<Option<f64>>>,
    bar_show: Rc<Cell<bool>>,
    idle_inhib: Rc<RefCell<Option<u32>>>,
    exit_after_current: Rc<Cell<bool>>,
}

fn wire_final_actions(ctx: FinalActionCtx) {
    let FinalActionCtx {
        app,
        win,
        root,
        gl,
        recent,
        bottom,
        player,
        sub_pref,
        video_pref,
        pref_menu,
        seek_bar_on,
        last_path,
        on_video_chrome,
        on_file_loaded,
        reapply_60,
        win_aspect,
        bar_show,
        idle_inhib,
        exit_after_current,
    } = ctx;

    let open = gio::SimpleAction::new("open", None);
    let p_open = player.clone();
    let gl_w = gl.clone();
    let recent_choose = recent.clone();
    let last_filepicker = last_path.clone();
    let ovc_open = on_video_chrome.clone();
    let wa_dlg = Rc::clone(&win_aspect);
    open.connect_activate(glib::clone!(
        #[weak]
        app,
        #[strong]
        ovc_open,
        #[strong]
        wa_dlg,
        #[strong]
        on_file_loaded,
        #[strong]
        reapply_60,
        move |_, _| {
            let Some(w) = app.active_window() else {
                return;
            };
            let vf = video_file_filter();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&vf);
            let dialog = gtk::FileDialog::builder()
                .title("Open video")
                .modal(true)
                .filters(&filters)
                .default_filter(&vf)
                .build();
            let p_c = p_open.clone();
            let w_f = w.clone();
            let gl_w = gl_w.clone();
            let recent_choose = recent_choose.clone();
            let last_fp = last_filepicker.clone();
            let ovc2 = ovc_open.clone();
            let wa2 = Rc::clone(&wa_dlg);
            let oload = Rc::clone(&on_file_loaded);
            let re_o = reapply_60.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else {
                    return;
                };
                let Some(path) = file.path() else {
                    eprintln!("[rhino] open: non-path URIs not implemented yet");
                    return;
                };
                let Some(aw) = w_f.downcast_ref::<adw::ApplicationWindow>() else {
                    return;
                };
                if let Err(e) = try_load(
                    &path,
                    &p_c,
                    aw,
                    &gl_w,
                    &recent_choose,
                    &LoadOpts {
                        record: true,
                        play_on_start: true,
                        last_path: last_fp.clone(),
                        on_start: Some(ovc2),
                        win_aspect: wa2.clone(),
                        on_loaded: Some(oload),
                        reapply_60: Some(re_o.clone()),
                    },
                ) {
                    eprintln!("[rhino] open: try_load: {e}");
                }
            });
        }
    ));
    app.add_action(&open);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let parent = app.active_window();
            let mut b = gtk::AboutDialog::builder()
                .program_name("Rhino Player")
                .version(env!("CARGO_PKG_VERSION"))
                .copyright("Copyright (C) 2026 Peter Adrianov")
                .logo_icon_name(APP_ID)
                .comments("mpv with GTK 4 and libadwaita.")
                .license(LICENSE_NOTICE)
                .license_type(gtk::License::Custom)
                .website("https://github.com/adrianov/rhino-player")
                .modal(true);
            if let Some(ref w) = parent {
                b = b.transient_for(w);
            }
            b.build().present();
        }
    ));
    app.add_action(&about);

    let app_q = app.clone();
    let quit = gio::SimpleAction::new("quit", None);
    let p_quit = player.clone();
    let win_q = win.clone();
    let sp_quit = sub_pref.clone();
    let idle_q = Rc::clone(&idle_inhib);
    quit.connect_activate(glib::clone!(
        #[strong]
        app_q,
        #[strong]
        p_quit,
        #[strong]
        win_q,
        #[strong]
        sp_quit,
        #[strong]
        idle_q,
        move |_, _| {
            schedule_quit_persist(&app_q, &win_q, &p_quit, &sp_quit, &idle_q);
        }
    ));
    app.add_action(&quit);

    let exit_after = gio::SimpleAction::new_stateful(
        "exit-after-current",
        None,
        &exit_after_current.get().to_variant(),
    );
    {
        let flag = Rc::clone(&exit_after_current);
        exit_after.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(on) = s.get::<bool>() else {
                return;
            };
            flag.set(on);
            a.set_state(s);
        });
    }
    app.add_action(&exit_after);

    register_video_app_actions(
        &app,
        &win,
        &gl,
        &player,
        Rc::clone(&video_pref),
        &pref_menu,
        Rc::clone(&seek_bar_on),
    );

    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.close-video", &["<Primary>w"]);
    app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete"]);
    app.set_accels_for_action("app.about", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Primary>q", "q"]);

    {
        let p = player.clone();
        let w = win.clone();
        let sp_close = sub_pref.clone();
        let iclose = Rc::clone(&idle_inhib);
        win.connect_close_request(glib::clone!(
            #[strong]
            app_q,
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            sp_close,
            #[strong]
            iclose,
            move |_win| {
                schedule_quit_persist(&app_q, &w, &p, &sp_close, &iclose);
                glib::Propagation::Stop
            }
        ));
    }

    apply_chrome(&root, &gl, &bar_show, &recent, &bottom, &player);
    {
        let pz = player.clone();
        let bz = bar_show.clone();
        let rz = recent.clone();
        let botz = bottom.clone();
        let glz = gl.clone();
        let on_sz = Rc::new(move || {
            if let Some(b) = pz.borrow().as_ref() {
                let show = if rz.is_visible() { true } else { bz.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, botz.height(), glz.height());
            }
        });
        let a = Rc::clone(&on_sz);
        let b = on_sz;
        gl.connect_notify_local(Some("height"), move |_, _| a());
        bottom.connect_notify_local(Some("height"), move |_, _| b());
    }

    {
        let idle_t = Rc::clone(&idle_inhib);
        let p_t = Rc::clone(&player);
        let r_t = recent.clone();
        let a_t = app.clone();
        let w_t = win.clone();
        glib::source::timeout_add_local(
            Duration::from_millis(500),
            glib::clone!(
                #[strong]
                a_t,
                #[strong]
                w_t,
                #[strong]
                p_t,
                #[strong]
                r_t,
                #[strong]
                idle_t,
                move || {
                    let should = idle_inhibit::should_inhibit(&p_t, r_t.is_visible());
                    let gtk_a: &gtk::Application = a_t.upcast_ref();
                    idle_inhibit::sync(gtk_a, Some(&w_t), should, &idle_t);
                    glib::ControlFlow::Continue
                }
            ),
        );
    }

    win.present();
}
