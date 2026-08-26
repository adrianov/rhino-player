thread_local! {
    /// Set by [wire_transport_events] when the mpv bundle is not ready yet.
    /// Invoked by [trigger_transport_install] from the GLArea realize path once the bundle exists.
    static TRANSPORT_INSTALL: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

thread_local! {
    /// [try_load] calls this after `loadfile` so `FileLoaded` / `path` / `duration` reach the transport
    /// UI without waiting for the next libmpv wakeup (continue grid + **Previous** could otherwise leave
    /// the clock and seek bar on the old title until user interaction).
    static TRANSPORT_DRAIN: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

thread_local! {
    /// Set by [wire_transport_events]. Seek / keyframe tails and **unpause** schedule the same debounced
    /// [schedule_smooth_60_resync_idle] as `FileLoaded` so Smooth is not applied twice in one interaction.
    static REQUEST_SMOOTH_60_RESYNC: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static CANCEL_SMOOTH_60_RESYNC: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Coalesce Smooth 60 / VapourSynth rebuild with transport (same timer as `FileLoaded` / `path` churn).
pub(crate) fn request_smooth_60_transport_resync() {
    REQUEST_SMOOTH_60_RESYNC.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

/// Cancel a pending debounced Smooth rebuild (call before a direct **`apply_mpv_video`** from the menu).
pub(crate) fn cancel_smooth_60_transport_resync() {
    CANCEL_SMOOTH_60_RESYNC.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

/// Drain libmpv events into [dispatch_event] immediately. Safe no-op before the GL realize hook runs.
pub(crate) fn transport_drain_after_loadfile() {
    TRANSPORT_DRAIN.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

/// Next main-loop turn — use after cross-chapter `loadfile` so `FileLoaded` is for the new VOB.
pub(crate) fn transport_drain_after_loadfile_idle() {
    let _ = glib::idle_add_local_once(transport_drain_after_loadfile);
}

thread_local! {
    /// One-shot [transport_tick] after warm resume / warm-hit open.
    static TRANSPORT_TICK: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Warm browse sync callback: DB resume + duration lookup plus prev/next wiring.
type WarmBrowseSync = Rc<dyn Fn(PathBuf)>;
thread_local! {
    /// Continue-grid hover / warm `loadfile` start: DB resume + duration, prev/next from [last_path].
    static WARM_BROWSE_SYNC: RefCell<Option<WarmBrowseSync>> = const { RefCell::new(None) };
}

thread_local! {
    /// After warm `FileLoaded` + resume idle — release [WarmPreloadGate] for the queued hover target.
    static WARM_PRELOAD_LOADED: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Refresh seek bar + clocks from mpv (and SQLite fallbacks on the continue grid).
pub(crate) fn transport_nudge_tick() {
    TRANSPORT_TICK.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

pub(crate) fn transport_sync_warm_browse(path: &Path) {
    let Some(canon) = std::fs::canonicalize(path).ok() else {
        return;
    };
    WARM_BROWSE_SYNC.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f(canon);
        }
    });
}

pub(crate) fn register_warm_preload_loaded(done: Rc<dyn Fn()>) {
    WARM_PRELOAD_LOADED.with(|slot| *slot.borrow_mut() = Some(done));
}

pub(crate) fn warm_preload_notify_loaded() {
    disarm_warm_path_settle();
    let done = WARM_PRELOAD_LOADED.with(|slot| slot.borrow().clone());
    if let Some(f) = done {
        let _ = glib::idle_add_local_once(move || f());
    }
}

/// Called from `wire_mpv_realize` right after the mpv bundle is created, so transport-event
/// observers attach without polling. No-op if observers were already installed.
fn trigger_transport_install() {
    let cb = TRANSPORT_INSTALL.with(|s| s.borrow_mut().take());
    if let Some(cb) = cb {
        cb();
    }
}
