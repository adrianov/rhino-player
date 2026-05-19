/// CLI path for first launch; ignore macOS `-psn_*` and other non-media argv tails.
fn boot_path_from_argv() -> Option<PathBuf> {
    let p = std::env::args().nth(1).map(PathBuf::from)?;
    if crate::video_ext::is_video_path(&p) || p.is_file() {
        Some(p)
    } else {
        None
    }
}

fn preload_first_continue(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> bool {
    if !recent.is_visible() || last_path.borrow().is_some() {
        return false;
    }
    let path = match history::load().into_iter().next() {
        Some(p) => p,
        None => return false,
    };
    let o = LoadOpts {
        video_pref: Rc::clone(video_pref),
        record: false,
        play_on_start: false,
        last_path: Rc::clone(last_path),
        on_start: None,
        win_aspect: Rc::new(Cell::new(None)),
        on_loaded: None,
        reset_speed_to_normal: false,
        hdr_title_mirror: None,
        playback_focus: None,
    };
    if load_file_into_player(&path, player, recent, &o).is_err() {
        return false;
    }
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }
    *last_path.borrow_mut() = std::fs::canonicalize(&path).ok();
    // Drain only after releasing `borrow_mut` — transport uses `try_borrow_mut` and would skip
    // `FileLoaded` / duration while the preload hold is active (seek bar stuck at 0:00 / 0:00).
    transport_drain_after_loadfile();
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);

    let player_i = player.clone();
    glib::idle_add_local_once(move || finish_preload_after_file_loaded(player_i));
    true
}

fn finish_preload_after_file_loaded(player: Rc<RefCell<Option<MpvBundle>>>) {
    transport_drain_after_loadfile();
    let _player = player;
}

fn schedule_preload_pause(player: Rc<RefCell<Option<MpvBundle>>>, recent: gtk::Box) {
    let _ = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        if recent.is_visible() {
            if let Some(b) = player.borrow().as_ref() {
                let _ = b.mpv.set_property("pause", true);
            }
        }
        glib::ControlFlow::Break
    });
}

/// Warm-preload the first continue entry after transport observers are installed.
fn run_continue_warm_preload(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    recent: &gtk::Box,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    _reapply_60: &VideoReapply60,
    skip_followups: bool,
) {
    if !preload_first_continue(player, video_pref, recent, last_path) {
        return;
    }
    if skip_followups {
        return;
    }
    schedule_preload_pause(player.clone(), recent.clone());
}

pub fn run() -> i32 {
    if std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--version" | "-V"))
    {
        println!("rhino-player {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }

    crate::glib_log_filter::install();

    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }

    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return 1;
    }

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|app| {
        glib::set_application_name(APP_WIN_TITLE);
        icons::register_hicolor_from_manifest();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        db::init();
        theme::apply();
        #[cfg(target_os = "macos")]
        crate::window_present::wire_activation_present(app);
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            let a = app.clone();
            glib::unix_signal_add_local(sig, move || {
                a.activate_action("quit", None);
                glib::ControlFlow::Break
            });
        }
    });

    let player: Rc<RefCell<Option<MpvBundle>>> = Rc::new(RefCell::new(None));
    let file_boot: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let on_open_slot: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    {
        let fb = Rc::clone(&file_boot);
        let slot = Rc::clone(&on_open_slot);
        let p_open = Rc::clone(&player);
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
                    *file_boot.borrow_mut() = boot_path_from_argv();
                }
                build_window(a, &p, Rc::clone(&file_boot), Rc::clone(&on_open_slot));
            }
        });
    }
    app.run().into()
}
