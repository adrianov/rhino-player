fn wire_final_open_dialog(ctx: &FinalActionCtx) {
    let open = gio::SimpleAction::new("open", None);
    let p_open = ctx.player.clone();
    let gl_w = ctx.gl.clone();
    let recent_choose = ctx.recent.clone();
    let last_filepicker = ctx.last_path.clone();
    let ovc_open = ctx.on_video_chrome.clone();
    let wa_dlg = Rc::clone(&ctx.win_aspect);
    let on_file_loaded_o = Rc::clone(&ctx.on_file_loaded);
    let hdr_mirror_o = ctx.hdr_title_mirror.clone();
    let app = ctx.app.clone();
    let playback_open = Rc::clone(&ctx.playback_focus);
    let video_pref_open = Rc::clone(&ctx.video_pref);

    open.connect_activate(glib::clone!(
        #[weak]
        app,
        #[strong]
        ovc_open,
        #[strong]
        wa_dlg,
        #[strong]
        on_file_loaded_o,
        #[strong]
        hdr_mirror_o,
        #[strong]
        playback_open,
        #[strong]
        video_pref_open,
        move |_, _| {
            let Some(w) = app.active_window() else {
                return;
            };
            let Some(aw) = w.clone().downcast::<adw::ApplicationWindow>().ok() else {
                return;
            };
            let p_c = p_open.clone();
            let gl_w = gl_w.clone();
            let recent_choose = recent_choose.clone();
            let last_fp = last_filepicker.clone();
            let ovc2 = ovc_open.clone();
            let wa2 = Rc::clone(&wa_dlg);
            let oload = Rc::clone(&on_file_loaded_o);
            let mirror_pick = hdr_mirror_o.clone();
            let pf_pick = Rc::clone(&playback_open);
            let vp_pick = video_pref_open.clone();
            let aw_load = aw.clone();
            let on_path = move |path: Option<std::path::PathBuf>| {
                let Some(path) = path else {
                    return;
                };
                if !crate::video_ext::is_openable_media_path(&path) {
                    eprintln!(
                        "[rhino] open: not a video file or optical-disc folder: {}",
                        path.display()
                    );
                    return;
                }
                let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
                    video_pref: Rc::clone(&vp_pick),
                    last_path: last_fp.clone(),
                    on_start: Some(ovc2),
                    win_aspect: wa2.clone(),
                    on_loaded: Some(oload),
                    play_on_start: true,
                    reset_speed_to_normal: false,
                    hdr_title_mirror: mirror_pick.clone(),
                });
                o.playback_focus = Some(Rc::clone(&pf_pick));
                if let Err(e) = try_load(&path, &p_c, &aw_load, &gl_w, &recent_choose, &o) {
                    eprintln!("[rhino] open: try_load: {e}");
                }
            };
            #[cfg(target_os = "macos")]
            {
                let _ = crate::macos_open_video::present_open_video_sheet(&aw, on_path);
                return;
            }
            #[cfg(not(target_os = "macos"))]
            {
                let vf = video_file_filter();
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&vf);
                let dialog = gtk::FileDialog::builder()
                    .title("Open Video")
                    .modal(true)
                    .filters(&filters)
                    .default_filter(&vf)
                    .build();
                dialog.open(Some(&aw), None::<&gio::Cancellable>, move |res| {
                    let Ok(file) = res else {
                        return;
                    };
                    on_path(file.path());
                });
            }
        }
    ));
    ctx.app.add_action(&open);
}

fn wire_final_about_dialog(ctx: &FinalActionCtx) {
    let about = gio::SimpleAction::new("about", None);
    let app = ctx.app.clone();
    about.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let parent = app.active_window();
            let mut b = gtk::AboutDialog::builder()
                .program_name("Rhino Player")
                .version(env!("CARGO_PKG_VERSION"))
                .copyright("Copyright © 2026 Peter Adrianov")
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
    ctx.app.add_action(&about);
}

fn wire_final_exit_after_toggle(ctx: &FinalActionCtx) {
    let exit_after = gio::SimpleAction::new_stateful(
        "exit-after-current",
        None,
        &ctx.exit_after_current.get().to_variant(),
    );
    let flag = Rc::clone(&ctx.exit_after_current);
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
    ctx.app.add_action(&exit_after);
}

fn wire_final_fullscreen_toggle(ctx: &FinalActionCtx) {
    let toggle_fullscreen_a = gio::SimpleAction::new("toggle-fullscreen", None);
    let w = ctx.win.clone();
    let fr = Rc::clone(&ctx.fs_restore);
    let lu = Rc::clone(&ctx.last_unmax);
    let sk = Rc::clone(&ctx.skip_max_to_fs);
    let fb = Rc::clone(&ctx.fs_transition_busy);
    toggle_fullscreen_a.connect_activate(move |_, _| {
        toggle_fullscreen(&w, fr.as_ref(), lu.as_ref(), &sk, fb.as_ref());
    });
    ctx.app.add_action(&toggle_fullscreen_a);
}

fn wire_final_platform_accels(ctx: &FinalActionCtx) {
    #[cfg(target_os = "macos")]
    {
        ctx.app.set_menubar(Some(&ctx.main_menu));
        ctx.app.set_accels_for_action("app.open", &["<Meta>o"]);
        ctx.app.set_accels_for_action("app.close-video", &["<Meta>w"]);
        ctx.app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete", "<Meta>BackSpace"]);
        ctx.app.set_accels_for_action("app.quit", &["<Meta>q", "q"]);
        ctx.app.set_accels_for_action("app.toggle-fullscreen", &["<Meta><Control>f"]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        ctx.app.set_accels_for_action("app.open", &["<Primary>o"]);
        ctx.app.set_accels_for_action("app.close-video", &["<Primary>w"]);
        ctx.app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete"]);
        ctx.app.set_accels_for_action("app.about", &["F1"]);
        ctx.app.set_accels_for_action("app.quit", &["<Primary>q", "q"]);
        ctx.app.set_accels_for_action("app.toggle-fullscreen", &["F11"]);
    }
}

fn wire_final_idle_chrome_resize(ctx: &FinalActionCtx) {
    apply_chrome(ChromeApplyParts {
        hdr_csd_baseline: &ctx.hdr_csd_baseline,
        root: &ctx.root,
        header: &ctx.header,
        gl: &ctx.gl,
        bar_show: &ctx.bar_show,
        recent: &ctx.recent,
        bottom: &ctx.bottom,
        player: &ctx.player,
    });
    wire_smooth_resize_and_subtitle_pos(
        &ctx.gl,
        &ctx.bottom,
        &ctx.player,
        &ctx.bar_show,
        &ctx.recent,
    );
    let idle_t = Rc::clone(&ctx.idle_inhib);
    let p_t = Rc::clone(&ctx.player);
    let r_t = ctx.recent.clone();
    let a_t = ctx.app.clone();
    let w_t = ctx.win.clone();
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
