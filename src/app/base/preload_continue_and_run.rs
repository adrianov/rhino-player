/// CLI path for first launch; ignore macOS `-psn_*` and other non-media argv tails.
fn boot_path_from_argv() -> Option<PathBuf> {
    let p = std::env::args().nth(1).map(PathBuf::from)?;
    if crate::video_ext::is_video_path(&p) || p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// At most one warm `loadfile` at a time; while busy, [WarmPreloadGate::queue] keeps **only** the latest path.
struct WarmPreloadGate {
    inflight: Cell<bool>,
    queued: RefCell<Option<PathBuf>>,
}

impl WarmPreloadGate {
    fn begin(&self) -> bool {
        if self.inflight.get() {
            return false;
        }
        self.inflight.set(true);
        true
    }

    /// Replace any prior queued hover target with [path] (last wins).
    fn queue(&self, path: PathBuf) {
        *self.queued.borrow_mut() = Some(path);
    }

    fn finish(&self, run_queued: impl FnOnce(PathBuf)) {
        self.inflight.set(false);
        if let Some(path) = self.queued.borrow_mut().take() {
            run_queued(path);
        }
    }
}

fn is_warm_file_loaded(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> bool {
    if !last_path
        .borrow()
        .as_ref()
        .is_some_and(|p| same_open_target(p, path))
    {
        return false;
    }
    player.borrow().as_ref().is_some_and(|b| {
        local_file_from_mpv(&b.mpv).is_some_and(|m| same_open_target(&m, path))
    })
}

struct WarmPreloadCtx {
    gate: Rc<WarmPreloadGate>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
}

impl WarmPreloadCtx {
    fn is_already_warm_loaded(&self, path: &Path) -> bool {
        is_warm_file_loaded(path, &self.player, &self.last_path)
    }

    fn run_path(&self, path: PathBuf) {
        if self.is_already_warm_loaded(&path) {
            return;
        }
        if !self.gate.begin() {
            self.gate.queue(path);
            return;
        }
        if !preload_continue_path(
            &path,
            &self.player,
            &self.video_pref,
            &self.recent,
            &self.last_path,
        ) {
            self.gate.finish(|p| self.run_path(p));
            return;
        }
        let ctx_i = Rc::new(WarmPreloadCtx {
            gate: Rc::clone(&self.gate),
            player: Rc::clone(&self.player),
            video_pref: Rc::clone(&self.video_pref),
            recent: self.recent.clone(),
            last_path: Rc::clone(&self.last_path),
        });
        glib::idle_add_local_once(move || finish_preload_after_file_loaded(ctx_i));
        schedule_preload_pause(Rc::clone(&self.player), self.recent.clone());
    }
}

fn preload_continue_path(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> bool {
    if !recent.is_visible() || !path.is_file() || player.borrow().is_none() {
        return false;
    }
    if is_warm_file_loaded(path, player, last_path) {
        return false;
    }
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
        warm_preload: true,
    };
    if load_file_into_player(path, player, recent, &o).is_err() {
        return false;
    }
    if let Some(b) = player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }
    *last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    transport_drain_after_loadfile();
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    true
}

fn preload_first_continue(ctx: &WarmPreloadCtx) -> bool {
    if !ctx.recent.is_visible() || ctx.last_path.borrow().is_some() {
        return false;
    }
    let path = match history::load().into_iter().next() {
        Some(p) => p,
        None => return false,
    };
    if !ctx.gate.begin() {
        ctx.gate.queue(path);
        return false;
    }
    if !preload_continue_path(
        &path,
        &ctx.player,
        &ctx.video_pref,
        &ctx.recent,
        &ctx.last_path,
    ) {
        ctx.gate.finish(|p| ctx.run_path(p));
        return false;
    }
    let ctx_i = Rc::new(WarmPreloadCtx {
        gate: Rc::clone(&ctx.gate),
        player: Rc::clone(&ctx.player),
        video_pref: Rc::clone(&ctx.video_pref),
        recent: ctx.recent.clone(),
        last_path: Rc::clone(&ctx.last_path),
    });
    glib::idle_add_local_once(move || finish_preload_after_file_loaded(ctx_i));
    true
}

fn finish_preload_after_file_loaded(ctx: Rc<WarmPreloadCtx>) {
    transport_drain_after_loadfile();
    ctx.gate.finish(|path| ctx.run_path(path));
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

/// Warm-preload one continue entry paused behind the grid.
fn run_continue_warm_preload_path(path: &Path, ctx: &Rc<WarmPreloadCtx>) {
    ctx.run_path(path.to_path_buf());
}

/// Warm-preload the first continue entry after transport observers are installed.
fn run_continue_warm_preload(
    ctx: &Rc<WarmPreloadCtx>,
    _reapply_60: &VideoReapply60,
    skip_followups: bool,
) {
    if !preload_first_continue(ctx) {
        return;
    }
    if skip_followups {
        ctx.gate.finish(|_| ());
        return;
    }
    schedule_preload_pause(Rc::clone(&ctx.player), ctx.recent.clone());
}

type WarmHoverLeave = Rc<dyn Fn()>;

/// Debounced enter/leave hooks for background warm preload on continue-card hover.
fn warm_hover_hooks(
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
) -> recent_view::WarmHoverHooks {
    let ctx = Rc::new(WarmPreloadCtx {
        gate: Rc::new(WarmPreloadGate {
            inflight: Cell::new(false),
            queued: RefCell::new(None),
        }),
        player,
        video_pref,
        recent,
        last_path,
    });
    let pending = Rc::new(RefCell::new(None::<glib::SourceId>));
    let pending_leave = pending.clone();
    let leave: WarmHoverLeave = Rc::new(move || {
        drop_glib_source(pending_leave.as_ref());
    });
    let enter = Rc::new(move |path: &Path| {
        drop_glib_source(pending.as_ref());
        if ctx.is_already_warm_loaded(path) {
            return;
        }
        let path = path.to_path_buf();
        let ctx = Rc::clone(&ctx);
        let pending_done = pending.clone();
        let id = glib::timeout_add_local(HOVER_WARM_PRELOAD_DELAY, move || {
            // GLib removes this source on `Break`; drop the slot only (never `g_source_remove`).
            pending_done.borrow_mut().take();
            if ctx.is_already_warm_loaded(&path) {
                return glib::ControlFlow::Break;
            }
            run_continue_warm_preload_path(&path, &ctx);
            glib::ControlFlow::Break
        });
        *pending.borrow_mut() = Some(id);
    });
    recent_view::WarmHoverHooks { enter, leave }
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
