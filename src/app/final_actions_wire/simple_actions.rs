#[derive(Clone)]
struct OpenPickCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    on_video_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<WinAspectCell>,
    on_file_loaded: Rc<dyn Fn()>,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
    on_open_fail: Rc<dyn Fn(String)>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
}

fn open_pick_ctx(ctx: &FinalActionCtx) -> OpenPickCtx {
    OpenPickCtx {
        player: ctx.player.clone(),
        gl: ctx.gl.clone(),
        recent: ctx.recent.clone(),
        last_path: ctx.last_path.clone(),
        on_video_chrome: ctx.on_video_chrome.clone(),
        win_aspect: Rc::clone(&ctx.win_aspect),
        on_file_loaded: ctx.on_file_loaded.clone(),
        hdr_title_mirror: ctx.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&ctx.playback_focus),
        on_open_fail: Rc::clone(&ctx.on_open_fail),
        video_pref: Rc::clone(&ctx.video_pref),
    }
}

fn wire_final_open_dialog(ctx: &FinalActionCtx) {
    let open = gio::SimpleAction::new("open", None);
    let pick = open_pick_ctx(ctx);
    let app = ctx.app.clone();
    open.connect_activate(glib::clone!(
        #[weak]
        app,
        #[strong]
        pick,
        move |_, _| {
            crate::user_action_log::act("menu Open (file picker)");
            let Some(w) = app.active_window() else {
                return;
            };
            let Some(aw) = w.clone().downcast::<adw::ApplicationWindow>().ok() else {
                return;
            };
            let pick = pick.clone();
            let aw_pick = aw.clone();
            let on_path =
                move |path: Option<std::path::PathBuf>| open_picked_path(path, &aw_pick, &pick);
            #[cfg(target_os = "macos")]
            {
                let _ = crate::macos_open_video::present_open_video_sheet(&aw, on_path);
                return;
            }
            #[cfg(not(target_os = "macos"))]
            run_gtk_open_dialog(&aw, on_path);
        }
    ));
    ctx.app.add_action(&open);
}

fn open_picked_path(path: Option<PathBuf>, aw: &adw::ApplicationWindow, c: &OpenPickCtx) {
    let Some(path) = path else {
        return;
    };
    if !crate::video_ext::is_openable_media_path(&path) {
        eprintln!(
            "[rhino] open: not a video file or optical-disc folder: {}",
            path.display()
        );
        (c.on_open_fail)(crate::media_open_fail::msg::UNREADABLE_MEDIA.to_string());
        return;
    }
    let o = picked_load_opts(c);
    if let Err(e) = try_load(&path, &c.player, aw, &c.gl, &c.recent, &o) {
        eprintln!("[rhino] open: try_load: {e}");
    }
}

/// Builds the replace-media options for a video chosen through the open dialog.
fn picked_load_opts(c: &OpenPickCtx) -> LoadOpts {
    let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
        video_pref: Rc::clone(&c.video_pref),
        last_path: c.last_path.clone(),
        on_start: Some(Rc::clone(&c.on_video_chrome)),
        win_aspect: Rc::clone(&c.win_aspect),
        on_loaded: Some(Rc::clone(&c.on_file_loaded)),
        play_on_start: true,
        reset_speed_to_normal: false,
        hdr_title_mirror: c.hdr_title_mirror.clone(),
    });
    o.playback_focus = Some(Rc::clone(&c.playback_focus));
    o.on_open_fail = Some(Rc::clone(&c.on_open_fail));
    o
}

#[cfg(not(target_os = "macos"))]
fn run_gtk_open_dialog(
    aw: &adw::ApplicationWindow,
    on_path: impl Fn(Option<std::path::PathBuf>) + 'static,
) {
    let vf = video_file_filter();
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&vf);
    let dialog = gtk::FileDialog::builder()
        .title("Open Video")
        .modal(true)
        .filters(&filters)
        .default_filter(&vf)
        .build();
    dialog.open(Some(aw), None::<&gio::Cancellable>, move |res| {
        let Ok(file) = res else {
            return;
        };
        on_path(file.path());
    });
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
