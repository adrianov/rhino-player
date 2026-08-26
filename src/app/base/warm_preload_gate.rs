// Warm-preload gate: single in-flight `loadfile`, one queued path, watchdog, and the debounced
// path-settle fallback. Split out of warm_preload_idle.rs (include!'d into the same module scope).

/// One warm `loadfile` at a time; at most one path queued until the current title is fully loaded.
pub(crate) struct WarmPreloadGate {
    inflight: Cell<bool>,
    /// [MpvBundle::warm_file_gen] for the in-flight warm `loadfile`.
    inflight_gen: Cell<u32>,
    queued: RefCell<Option<PathBuf>>,
    watchdog: Rc<RefCell<Option<glib::SourceId>>>,
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

    /// User opened for playback — drop in-flight warm preload without running the queued hover target.
    pub(crate) fn cancel(&self) {
        self.disarm_watchdog();
        self.inflight.set(false);
        *self.queued.borrow_mut() = None;
    }

    pub(crate) fn arm_watchdog(&self, player: Rc<RefCell<Option<MpvBundle>>>, inflight_gen: u32) {
        self.disarm_watchdog();
        let wd = Rc::clone(&self.watchdog);
        *self.watchdog.borrow_mut() = Some(glib::timeout_add_local(
            Duration::from_millis(WARM_PRELOAD_WATCHDOG_MS),
            move || {
                crate::glib_source_drop::finish_glib_source(wd.as_ref());
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
        crate::glib_source_drop::drop_glib_source(self.watchdog.as_ref());
    }
}

const WARM_PRELOAD_WATCHDOG_MS: u64 = 4000;
const WARM_PATH_SETTLE_MS: u64 = 80;

pub(crate) fn disarm_warm_path_settle() {
    WARM_CTX.with(|s| {
        if let Some(c) = s.borrow().as_ref() {
            crate::glib_source_drop::drop_glib_source(c.path_settle.as_ref());
        }
    });
}

fn warm_path_settle_tick(
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
) -> glib::ControlFlow {
    crate::glib_source_drop::finish_glib_source(slot.as_ref());
    if !warm_preload_gate_busy() {
        return glib::ControlFlow::Break;
    }
    let want_gen = WARM_CTX.with(|s| {
        s.borrow()
            .as_ref()
            .map(|c| c.gate.inflight_gen())
            .unwrap_or(0)
    });
    let pending = Rc::clone(&player);
    let _ = glib::idle_add_local_once(move || {
        warm_preload_finish_load(&pending, want_gen);
    });
    glib::ControlFlow::Break
}

/// Grab the coalesced settle source slot from [WARM_CTX], dropping any pending one.
fn take_warm_settle_slot() -> Option<Rc<RefCell<Option<glib::SourceId>>>> {
    let slot = WARM_CTX.with(|s| s.borrow().as_ref().map(|c| Rc::clone(&c.path_settle)))?;
    crate::glib_source_drop::drop_glib_source(slot.as_ref());
    Some(slot)
}

/// Debounced fallback when `FileLoaded` is dropped during rapid hover `loadfile` churn.
pub(crate) fn schedule_warm_path_settle(player: Rc<RefCell<Option<MpvBundle>>>) {
    if !warm_preload_gate_busy() {
        return;
    }
    let Some(settle_slot) = take_warm_settle_slot() else {
        return;
    };
    let slot = Rc::clone(&settle_slot);
    *settle_slot.borrow_mut() = Some(glib::timeout_add_local(
        Duration::from_millis(WARM_PATH_SETTLE_MS),
        move || warm_path_settle_tick(&slot, Rc::clone(&player)),
    ));
}
