// Application bootstrap helpers for [run]: version flag, app construction, and startup/open/activate
// wiring. Split out of preload_continue_and_run.rs (include!'d into the same module scope).

/// CLI path for first launch; ignore macOS `-psn_*` and other non-media argv tails.
fn boot_path_from_argv() -> Option<PathBuf> {
    let p = std::env::args().nth(1).map(PathBuf::from)?;
    if crate::video_ext::is_openable_media_path(&p) {
        Some(p)
    } else {
        None
    }
}

/// `--version` / `-V`: print the crate version; returns the process exit code to use.
fn print_version_exit() -> Option<i32> {
    if std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--version" | "-V"))
    {
        println!("rhino-player {}", env!("CARGO_PKG_VERSION"));
        return Some(0);
    }
    None
}

fn build_adw_application() -> adw::Application {
    adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build()
}

/// Startup: app name/icons/theme/db init, macOS activation-present wiring, signals → quit action.
fn wire_app_startup(app: &adw::Application) {
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
}

/// Filter glib log spam, pin `LC_NUMERIC`, init libadwaita, build the app, wire startup.
/// Returns Err(exit code) when libadwaita cannot initialize.
fn bootstrap_app() -> Result<adw::Application, i32> {
    crate::glib_log_filter::install();
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return Err(1);
    }
    let app = build_adw_application();
    wire_app_startup(&app);
    Ok(app)
}

type AppStateSlots = (
    Rc<RefCell<Option<MpvBundle>>>,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<RefCell<Option<RcPathFn>>>,
);

fn new_app_state() -> AppStateSlots {
    (
        Rc::new(RefCell::new(None)),
        Rc::new(RefCell::new(None)),
        Rc::new(RefCell::new(None)),
    )
}

/// Route one opened media path: defer off the Apple Event handler while a player exists.
fn handle_app_open_path(
    app: &adw::Application,
    path: PathBuf,
    p_open: &Rc<RefCell<Option<MpvBundle>>>,
    fb: &Rc<RefCell<Option<PathBuf>>>,
    slot: &Rc<RefCell<Option<RcPathFn>>>,
) {
    if p_open.borrow().is_some() {
        // Never run try_load synchronously here: macOS Finder / "Open With" delivers
        // g_application_open during Apple Event handling; a nested player RefCell
        // borrow (transport drain, loadfile) aborts via panic_cannot_unwind.
        if let Some(f) = slot.borrow().clone() {
            glib::idle_add_local_once(move || f(&path));
        } else {
            *fb.borrow_mut() = Some(path);
        }
        return;
    }
    *fb.borrow_mut() = Some(path);
    if app.windows().is_empty() {
        app.activate();
    }
}

/// `Open` handling: stash the path for activate, or defer it off the Apple Event handler.
fn wire_app_open(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: &Rc<RefCell<Option<RcPathFn>>>,
) {
    let fb = Rc::clone(file_boot);
    let slot = Rc::clone(on_open_slot);
    let p_open = Rc::clone(player);
    app.connect_open(move |app, files, _| {
        let Some(path) = files.first().and_then(|f| f.path()) else {
            eprintln!("[rhino] open: no local path in file list");
            return;
        };
        handle_app_open_path(app, path, &p_open, &fb, &slot);
    });
}
/// Activate: seed the boot path from argv when absent, then build the single window.
fn wire_app_activate(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: &Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: &Rc<RefCell<Option<RcPathFn>>>,
) {
    let p = player.clone();

    let file_boot = Rc::clone(file_boot);
    let on_open_slot = Rc::clone(on_open_slot);
    app.connect_activate(move |a: &adw::Application| {
        if a.windows().is_empty() {
            if file_boot.borrow().is_none() {
                *file_boot.borrow_mut() = boot_path_from_argv();
            }
            build_window(a, &p, Rc::clone(&file_boot), Rc::clone(&on_open_slot));
        }
    });
}
