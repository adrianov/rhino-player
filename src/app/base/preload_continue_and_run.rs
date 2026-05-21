/// CLI path for first launch; ignore macOS `-psn_*` and other non-media argv tails.
fn boot_path_from_argv() -> Option<PathBuf> {
    let p = std::env::args().nth(1).map(PathBuf::from)?;
    if crate::video_ext::is_video_path(&p) || p.is_file() {
        Some(p)
    } else {
        None
    }
}

include!("warm_preload_idle.rs");

/// mpv already has this local file open (canonical path compare).
fn mpv_has_open_target(path: &Path, player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    let Ok(g) = player.try_borrow() else {
        return false;
    };
    g.as_ref().is_some_and(|b| {
        local_file_from_mpv(&b.mpv).is_some_and(|m| same_open_target(&m, path))
    })
}

pub(crate) struct WarmPreloadCtx {
    gate: Rc<WarmPreloadGate>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    /// Coalesced hover target; runs at [glib::Priority::LOW] so scroll/motion stays smooth.
    hover_idle: Rc<RefCell<Option<glib::SourceId>>>,
    path_settle: Rc<RefCell<Option<glib::SourceId>>>,
}

impl WarmPreloadCtx {
    pub(crate) fn new(
        player: Rc<RefCell<Option<MpvBundle>>>,
        video_pref: Rc<RefCell<db::VideoPrefs>>,
        recent: gtk::Box,
        last_path: Rc<RefCell<Option<PathBuf>>>,
    ) -> Rc<Self> {
        Rc::new(Self {
            gate: Rc::new(WarmPreloadGate {
                inflight: Cell::new(false),
                inflight_gen: Cell::new(0),
                queued: RefCell::new(None),
                watchdog: Rc::new(RefCell::new(None)),
            }),
            player,
            video_pref,
            recent,
            last_path,
            hover_idle: Rc::new(RefCell::new(None)),
            path_settle: Rc::new(RefCell::new(None)),
        })
    }

    /// Skip preload only when mpv already has this file **and** no other warm `loadfile` is in flight.
    fn warm_target_ready(&self, path: &Path) -> bool {
        mpv_has_open_target(path, &self.player) && !self.gate.busy()
    }

    fn run_path(ctx: &Rc<Self>, path: PathBuf) {
        if ctx.warm_target_ready(&path) && ctx.gate.queued.borrow().is_none() {
            return;
        }
        if !ctx.gate.try_begin() {
            ctx.gate.queue(path);
            return;
        }
        match preload_continue_path(
            &path,
            &ctx.player,
            &ctx.video_pref,
            &ctx.recent,
            &ctx.last_path,
        ) {
            PreloadOutcome::Deferred => {
                let gen = ctx
                    .player
                    .borrow()
                    .as_ref()
                    .map(crate::mpv_embed::MpvBundle::warm_file_gen)
                    .unwrap_or(0);
                ctx.gate.set_inflight_gen(gen);
                ctx.gate
                    .arm_watchdog(Rc::clone(&ctx.player), gen);
                schedule_preload_pause(Rc::clone(&ctx.player), ctx.recent.clone());
            }
            PreloadOutcome::Ready => {
                let run = Rc::clone(ctx);
                let gate = Rc::clone(&run.gate);
                let player = Rc::clone(&ctx.player);
                let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
                    finish_warm_preload_ready_now(&player);
                    let run = Rc::clone(&run);
                    gate.complete(move |p| Self::run_path(&run, p));
                    glib::ControlFlow::Break
                });
            }
            PreloadOutcome::Failed => {
                let run = Rc::clone(ctx);
                let gate = Rc::clone(&ctx.gate);
                gate.complete(move |p| Self::run_path(&run, p));
            }
        }
    }
}

include!("warm_preload_path.rs");

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
