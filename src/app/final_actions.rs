struct FinalActionCtx {
    app: adw::Application,
    win: adw::ApplicationWindow,
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    gl: gtk::GLArea,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    /// macOS global menu bar model (File / View); discarded on Linux.
    main_menu: gio::Menu,
    pref_menu: gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    bar_show: Rc<Cell<bool>>,
    idle_inhib: Rc<RefCell<Option<u32>>>,
    exit_after_current: Rc<Cell<bool>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
}

include!("final_actions_smooth_resize.rs");

fn wire_final_actions(ctx: FinalActionCtx) {
    let FinalActionCtx {
        app,
        win,
        fs_restore,
        last_unmax,
        skip_max_to_fs,
        root,
        header,
        gl,
        recent,
        bottom,
        player,
        sub_pref,
        video_pref,
        main_menu,
        pref_menu,
        seek_bar_on,
        last_path,
        on_video_chrome,
        on_file_loaded,
        win_aspect,
        bar_show,
        idle_inhib,
        exit_after_current,
        mpv_teardown_after_draw,
        hdr_csd_baseline,
        hdr_title_mirror,
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
        hdr_title_mirror,
        move |_, _| {
            let Some(w) = app.active_window() else {
                return;
            };
            let vf = video_file_filter();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&vf);
            let dialog = gtk::FileDialog::builder()
                .title("Open Video")
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
            let mirror_pick = hdr_title_mirror.clone();
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
                    &LoadOpts::replace_media(
                        last_fp.clone(),
                        Some(ovc2),
                        wa2.clone(),
                        Some(oload),
                        true,
                        false,
                        mirror_pick.clone(),
                    ),
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

    wire_quit_close(
        &app,
        &win,
        &gl,
        &player,
        &sub_pref,
        &idle_inhib,
        &mpv_teardown_after_draw,
    );

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

    let toggle_fullscreen_a = gio::SimpleAction::new("toggle-fullscreen", None);
    {
        let w = win.clone();
        let fr = Rc::clone(&fs_restore);
        let lu = Rc::clone(&last_unmax);
        let sk = Rc::clone(&skip_max_to_fs);
        toggle_fullscreen_a.connect_activate(move |_, _| {
            toggle_fullscreen(&w, fr.as_ref(), lu.as_ref(), sk.as_ref());
        });
    }
    app.add_action(&toggle_fullscreen_a);

    register_video_app_actions(
        &app,
        &win,
        &gl,
        &player,
        Rc::clone(&video_pref),
        &pref_menu,
        Rc::clone(&seek_bar_on),
    );

    #[cfg(target_os = "macos")]
    app.set_menubar(Some(&main_menu));
    #[cfg(not(target_os = "macos"))]
    drop(main_menu);

    #[cfg(target_os = "macos")]
    {
        // gdk-macos: Command (⌘) is [Meta]; Control keeps the separate hardware Ctrl for ⌃⌘F fullscreen.
        app.set_accels_for_action("app.open", &["<Meta>o"]);
        app.set_accels_for_action("app.close-video", &["<Meta>w"]);
        app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete", "<Meta>BackSpace"]);
        app.set_accels_for_action("app.quit", &["<Meta>q", "q"]);
        app.set_accels_for_action("app.toggle-fullscreen", &["<Meta><Control>f"]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        app.set_accels_for_action("app.open", &["<Primary>o"]);
        app.set_accels_for_action("app.close-video", &["<Primary>w"]);
        app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete"]);
        app.set_accels_for_action("app.about", &["F1"]);
        app.set_accels_for_action("app.quit", &["<Primary>q", "q"]);
        app.set_accels_for_action("app.toggle-fullscreen", &["F11"]);
    }

    apply_chrome(ChromeApplyParts {
        hdr_csd_baseline: &hdr_csd_baseline,
        root: &root,
        header: &header,
        gl: &gl,
        bar_show: &bar_show,
        recent: &recent,
        bottom: &bottom,
        player: &player,
    });
    wire_smooth_resize_and_subtitle_pos(
        &gl,
        &bottom,
        &player,
        &bar_show,
        &recent,
    );

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

include!("final_actions_quit_close.rs");
