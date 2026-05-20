/// One warm `loadfile` at a time; at most one path queued until the current title is fully loaded.
pub(crate) struct WarmPreloadGate {
    inflight: Cell<bool>,
    /// [MpvBundle::warm_file_gen] for the in-flight warm `loadfile`.
    inflight_gen: Cell<u32>,
    queued: RefCell<Option<PathBuf>>,
    watchdog: RefCell<Option<glib::SourceId>>,
}

impl WarmPreloadGate {
    pub(crate) fn try_begin(&self) -> bool {
        if self.inflight.get() {
            return false;
        }
        self.inflight.set(true);
        true
    }

    pub(crate) fn queue(&self, path: PathBuf) {
        *self.queued.borrow_mut() = Some(path);
    }

    pub(crate) fn set_inflight_gen(&self, gen: u32) {
        self.inflight_gen.set(gen);
    }

    pub(crate) fn inflight_gen(&self) -> u32 {
        self.inflight_gen.get()
    }

    pub(crate) fn complete(&self, run_queued: impl FnOnce(PathBuf) + 'static) {
        self.disarm_watchdog();
        self.inflight.set(false);
        if let Some(path) = self.queued.borrow_mut().take() {
            let _ = glib::idle_add_local_once(move || run_queued(path));
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.inflight.get()
    }

    pub(crate) fn arm_watchdog(
        &self,
        player: Rc<RefCell<Option<MpvBundle>>>,
        inflight_gen: u32,
    ) {
        self.disarm_watchdog();
        *self.watchdog.borrow_mut() = Some(glib::timeout_add_local(
            Duration::from_millis(WARM_PRELOAD_WATCHDOG_MS),
            move || {
                if warm_preload_gate_busy() {
                    eprintln!("[rhino] warm preload: watchdog release");
                    let player = Rc::clone(&player);
                    let _ = glib::idle_add_local_once(move || {
                        warm_preload_finish_load(&player, inflight_gen);
                    });
                }
                glib::ControlFlow::Break
            },
        ));
    }

    fn disarm_watchdog(&self) {
        crate::glib_source_drop::drop_glib_source(&self.watchdog);
    }
}

const WARM_PRELOAD_WATCHDOG_MS: u64 = 4000;
const WARM_PATH_SETTLE_MS: u64 = 80;

thread_local! {
    static WARM_CTX: RefCell<Option<Rc<WarmPreloadCtx>>> = const { RefCell::new(None) };
}

/// Whether hover preload started an async `loadfile` or finished synchronously.
enum PreloadOutcome {
    Deferred,
    Ready,
    Failed,
}

pub(crate) fn register_warm_preload_ctx(ctx: Rc<WarmPreloadCtx>) {
    WARM_CTX.with(|s| *s.borrow_mut() = Some(ctx));
}

pub(crate) fn warm_preload_gate_busy() -> bool {
    WARM_CTX.with(|s| s.borrow().as_ref().is_some_and(|c| c.gate.busy()))
}

pub(crate) fn disarm_warm_path_settle() {
    WARM_CTX.with(|s| {
        if let Some(c) = s.borrow().as_ref() {
            crate::glib_source_drop::drop_glib_source(c.path_settle.as_ref());
        }
    });
}

/// Debounced fallback when `FileLoaded` is dropped during rapid hover `loadfile` churn.
pub(crate) fn schedule_warm_path_settle(player: Rc<RefCell<Option<MpvBundle>>>) {
    if !warm_preload_gate_busy() {
        return;
    }
    let settle_slot = WARM_CTX.with(|s| {
        s.borrow()
            .as_ref()
            .map(|c| Rc::clone(&c.path_settle))
    });
    let Some(settle_slot) = settle_slot else {
        return;
    };
    crate::glib_source_drop::drop_glib_source(settle_slot.as_ref());
    let slot = Rc::clone(&settle_slot);
    *settle_slot.borrow_mut() = Some(glib::timeout_add_local(
        Duration::from_millis(WARM_PATH_SETTLE_MS),
        move || {
            crate::glib_source_drop::drop_glib_source(slot.as_ref());
            if !warm_preload_gate_busy() {
                return glib::ControlFlow::Break;
            }
            let want_gen = WARM_CTX.with(|s| {
                s.borrow()
                    .as_ref()
                    .map(|c| c.gate.inflight_gen())
                    .unwrap_or(0)
            });
            let player = Rc::clone(&player);
            let _ = glib::idle_add_local_once(move || {
                warm_preload_finish_load(&player, want_gen);
            });
            glib::ControlFlow::Break
        },
    ));
}

pub(crate) fn warm_preload_finish_load(player: &Rc<RefCell<Option<MpvBundle>>>, want_gen: u32) {
    let cur = match player.try_borrow() {
        Ok(g) => g
            .as_ref()
            .map(crate::mpv_embed::MpvBundle::warm_file_gen)
            .unwrap_or(0),
        Err(_) => {
            let p = Rc::clone(player);
            let _ = glib::idle_add_local_once(move || warm_preload_finish_load(&p, want_gen));
            return;
        }
    };
    if cur != want_gen {
        warm_preload_notify_loaded();
        return;
    }
    warm_preload_apply_resume_audio(player);
    transport_nudge_tick();
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    warm_preload_notify_loaded();
}

fn warm_preload_apply_resume_audio(player: &Rc<RefCell<Option<MpvBundle>>>) {
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            b.apply_pending_resume();
            crate::audio_tracks::restore_saved_audio(&b.mpv);
            crate::audio_tracks::ensure_playable_audio(&b.mpv);
        }
    }
}

fn finish_warm_preload_ready_now(player: &Rc<RefCell<Option<MpvBundle>>>) {
    let Ok(g) = player.try_borrow() else {
        let p = Rc::clone(player);
        let _ = glib::idle_add_local_once(move || finish_warm_preload_ready_now(&p));
        return;
    };
    if let Some(b) = g.as_ref() {
        b.apply_pending_resume();
        crate::audio_tracks::restore_saved_audio(&b.mpv);
        crate::audio_tracks::ensure_playable_audio(&b.mpv);
        let _ = b.mpv.set_property("pause", true);
    }
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
    transport_nudge_tick();
}

fn schedule_warm_hover_preload(ctx: &Rc<WarmPreloadCtx>, path: PathBuf) {
    crate::glib_source_drop::drop_glib_source(&ctx.hover_idle);
    let run = Rc::clone(ctx);
    *ctx.hover_idle.borrow_mut() = Some(glib::source::idle_add_local_full(
        glib::Priority::LOW,
        move || {
            *run.hover_idle.borrow_mut() = None;
            run_continue_warm_preload_path(&path, &run);
            glib::ControlFlow::Break
        },
    ));
}

fn run_continue_warm_preload_path(path: &Path, ctx: &Rc<WarmPreloadCtx>) {
    transport_sync_warm_browse(path);
    if ctx.warm_target_ready(path) && ctx.gate.queued.borrow().is_none() {
        return;
    }
    WarmPreloadCtx::run_path(ctx, path.to_path_buf());
}
