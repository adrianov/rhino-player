fn preload_first_continue(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> Option<bool> {
    if !recent.is_visible() || last_path.borrow().is_some() {
        return None;
    }
    let path = history::load().into_iter().next()?;
    let mut p = player.borrow_mut();
    let b = p.as_mut()?;
    let _ = b.mpv.set_property("pause", true);
    b.load_file_path(&path, false).ok()?;
    let _ = b.mpv.set_property("pause", true);
    *last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
    Some(video_pref::apply_mpv_video(&b.mpv, &mut video.borrow_mut(), None).smooth_auto_off)
}

pub fn run() -> i32 {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }

    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return 1;
    }

    // Without HANDLES_OPEN, the desktop/portal rejects opening files: "This application can not open files"
    // (https://github.com/gtk-rs/gtk4-rs/issues/1039) — `open` is used instead of argv[1].
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|app| {
        icons::register_hicolor_from_manifest();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        db::init();
        theme::apply();
        // Route termination signals through the GLib main loop so the normal quit path runs
        // (saves resume position, stops mpv) instead of instant process death.
        // SIGKILL cannot be caught.
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            let a = app.clone();
            glib::unix_signal_add_local(sig, move || {
                a.activate_action("quit", None);
                glib::ControlFlow::Break
            });
        }
    });

    let player: Rc<RefCell<Option<MpvBundle>>> = Rc::new(RefCell::new(None));
    // Queued for first GL init ([connect_realize]) or applied via [on_open] when libmpv is ready.
    let file_boot: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let on_open_slot: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    {
        let fb = Rc::clone(&file_boot);
        let slot = Rc::clone(&on_open_slot);
        let p_open = Rc::clone(&player);
        // With HANDLES_OPEN, the default handler does **not** emit `activate` when argv lists files —
        // only `open` (see g_application_run: files → `open` signal). Without a call to
        // `Gio::Application::activate`, no window is created and the process exits (use count 0).
        app.connect_open(move |app, files, _| {
            let path = match files.first().and_then(|f| f.path()) {
                Some(p) => p,
                None => return,
            };
            if p_open.borrow().is_some() {
                if let Some(f) = slot.borrow().as_ref() {
                    f(&path);
                } else {
                    *fb.borrow_mut() = Some(path);
                }
                return;
            }
            *fb.borrow_mut() = Some(path);
            if app.windows().is_empty() {
                app.activate();
            }
        });
    }
    {
        let p = player.clone();
        let file_boot = Rc::clone(&file_boot);
        let on_open_slot = Rc::clone(&on_open_slot);
        app.connect_activate(move |a: &adw::Application| {
            if a.windows().is_empty() {
                if file_boot.borrow().is_none() {
                    if let Some(arg) = std::env::args().nth(1) {
                        *file_boot.borrow_mut() = Some(PathBuf::from(arg));
                    }
                }
                build_window(a, &p, Rc::clone(&file_boot), Rc::clone(&on_open_slot));
            }
        });
    }
    app.run().into()
}
