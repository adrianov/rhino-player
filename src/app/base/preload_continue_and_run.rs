include!("app_bootstrap.rs");

include!("warm_preload_idle.rs");

/// mpv already has this local file open (canonical path compare).
fn mpv_has_open_target(path: &Path, player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    let Ok(g) = player.try_borrow() else {
        return false;
    };
    g.as_ref()
        .is_some_and(|b| crate::media_probe::mpv_warm_hit_ready(&b.mpv, path))
}

pub(crate) struct WarmPreloadCtx {
    gate: Rc<WarmPreloadGate>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    recent: gtk::Box,
    gl: gtk::GLArea,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    path_settle: Rc<RefCell<Option<glib::SourceId>>>,
}

impl WarmPreloadCtx {
    pub(crate) fn new(
        player: Rc<RefCell<Option<MpvBundle>>>,
        video_pref: Rc<RefCell<db::VideoPrefs>>,
        recent: gtk::Box,
        gl: gtk::GLArea,
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
            gl,
            last_path,
            path_settle: Rc::new(RefCell::new(None)),
        })
    }

    /// Skip preload only when mpv already has this file **and** no other warm `loadfile` is in flight.
    fn warm_target_ready(&self, path: &Path) -> bool {
        mpv_has_open_target(path, &self.player) && !self.gate.busy()
    }

    fn run_path(ctx: &Rc<Self>, path: PathBuf) {
        if ctx.warm_target_ready(&path) && ctx.gate.queued.borrow().is_none() {
            warm_preload_hold_browse_pause(&ctx.player, &ctx.gl);
            return;
        }
        if !ctx.gate.try_begin() {
            ctx.gate.queue(path);
            return;
        }
        settle_preload_outcome(
            ctx,
            preload_continue_path(
                &path,
                &ctx.player,
                &ctx.video_pref,
                &ctx.recent,
                &ctx.gl,
                &ctx.last_path,
            ),
        );
    }
}

/// Shared disposition of a [PreloadOutcome] from [preload_continue_path]: arm the watchdog and
/// pause tick on [PreloadOutcome::Deferred], finish chrome sync then drain the queue on
/// [PreloadOutcome::Ready], release the gate on [PreloadOutcome::Failed].
/// Returns `true` only for [PreloadOutcome::Deferred] (a warm load actually started).
fn settle_preload_outcome(ctx: &Rc<WarmPreloadCtx>, outcome: PreloadOutcome) -> bool {
    match outcome {
        PreloadOutcome::Deferred => {
            arm_deferred_warm_load(ctx);
            true
        }
        PreloadOutcome::Ready => {
            finish_ready_then_drain_queue(ctx);
            false
        }
        PreloadOutcome::Failed => {
            let run = Rc::clone(ctx);
            Rc::clone(&ctx.gate).complete(move |p| WarmPreloadCtx::run_path(&run, p));
            false
        }
    }
}

/// [PreloadOutcome::Deferred] arm: latch the in-flight gen, arm the watchdog and the pause tick.
fn arm_deferred_warm_load(ctx: &Rc<WarmPreloadCtx>) {
    let gen = ctx
        .player
        .borrow()
        .as_ref()
        .map(crate::mpv_embed::MpvBundle::warm_file_gen)
        .unwrap_or(0);
    ctx.gate.set_inflight_gen(gen);
    ctx.gate.arm_watchdog(Rc::clone(&ctx.player), gen);
    schedule_preload_pause(Rc::clone(&ctx.player), ctx.gl.clone());
}

/// [PreloadOutcome::Ready] arm: finish chrome sync on a low-priority idle, then drain the queue.
fn finish_ready_then_drain_queue(ctx: &Rc<WarmPreloadCtx>) {
    let player = Rc::clone(&ctx.player);
    let gl = ctx.gl.clone();
    let run = Rc::clone(ctx);
    let gate = Rc::clone(&run.gate);
    let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
        finish_warm_preload_ready_now(&player, &gl);
        let run = Rc::clone(&run);
        gate.complete(move |p| WarmPreloadCtx::run_path(&run, p));
        glib::ControlFlow::Break
    });
}

include!("warm_preload_path.rs");

pub fn run() -> i32 {
    if let Some(code) = print_version_exit() {
        return code;
    }
    let Ok(app) = bootstrap_app() else {
        return 1;
    };
    let (player, file_boot, on_open_slot) = new_app_state();
    wire_app_open(&app, &player, &file_boot, &on_open_slot);
    wire_app_activate(&app, &player, &file_boot, &on_open_slot);
    app.run().into()
}
