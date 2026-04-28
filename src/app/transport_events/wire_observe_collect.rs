// Transport / volume / mute / EOF wiring.
//
// Property observation is used for state that changes on user/UI action (pause, duration, volume,
// mute, volume-max, path) so the UI updates immediately. Time-pos, core-idle, eof-reached, and
// speed are sampled by [transport_tick] every second instead — libmpv property-change events for
// those are unreliable at high playback speed (see `docs/features/04-transport-and-progress.md`,
// `events-over-polling.mdc`: this is a documented fallback when no reliable event exists).
//
// The 1-second tick also handles **sibling auto-advance** on natural EOF (see [docs/features/07-sibling-folder-queue.md]).
// Diagnostics: set `RHINO_TRANSPORT_TRACE=1` to print each dispatched event to stderr.

const PROP_PAUSE: u64 = 1;
const PROP_DURATION: u64 = 2;
const PROP_VOLUME: u64 = 3;
const PROP_MUTE: u64 = 4;
const PROP_VOLUME_MAX: u64 = 5;
const PROP_PATH: u64 = 6;

/// State + UI tick. 1 Hz is enough for the time labels and seek-bar thumb at any speed; sibling
/// advance fires within a second of mpv reaching `core-idle` near the end.
const TICK_INTERVAL: Duration = Duration::from_secs(1);
/// Seconds before `duration` where `core-idle=true` is treated as natural EOF (decoder stall near
/// the tail, including high playback speed).
const TICK_EOF_TAIL_SEC: f64 = 1.5;

#[derive(Clone, Debug)]
enum TransportEv {
    Pause(bool),
    Duration(f64),
    Volume(f64),
    Mute(bool),
    VolumeMax(f64),
    FileLoaded,
    VideoReconfig,
    /// `path` changed; consumers re-read mpv to fetch the up-to-date file path.
    PathChanged,
}

struct TransportWidgets {
    play_pause: gtk::Button,
    seek: gtk::Scale,
    seek_adj: gtk::Adjustment,
    seek_sync: Rc<Cell<bool>>,
    /// True while the user is pressing the seek thumb (mouse / touch). The 1 Hz tick skips
    /// programmatic position writes so dragging the thumb is not interrupted.
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    time_right: gtk::Label,
    speed_menu: gtk::MenuButton,
    vol_menu: gtk::MenuButton,
    vol_adj: gtk::Adjustment,
    vol_mute: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
}

#[derive(Default)]
struct TransportCache {
    duration: f64,
    pause: bool,
    pos: f64,
    /// True when mpv playback core is not progressing (EOF with `keep-open=yes`, buffering, seeking, stalled).
    core_idle: bool,
}

struct TransportEofCtx {
    app: adw::Application,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    idle_inhib: Rc<RefCell<Option<u32>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
}

struct TransportCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    widgets: TransportWidgets,
    eof: TransportEofCtx,
    /// Bottom-bar visibility flag; transient seek-slider redraws are skipped while it is `false`
    /// to avoid invalidating chrome that is animating in / out (the cause of fullscreen flicker).
    bar_show: Rc<Cell<bool>>,
    /// Toggled to keep the recent grid path in sync; if `recent` is visible the seek bar is hidden too.
    recent_visible: Rc<Cell<bool>>,
    sibling_nav: SiblingNavUi,
    /// Coalesce [glib::idle_add_local_once] resyncs on `FileLoaded` / `path` churn.
    idle_resync_pending: Rc<Cell<bool>>,
    /// 1 Hz timer source id (kept so it can be replaced if observers re-install).
    tick: Rc<RefCell<Option<glib::SourceId>>>,
    cache: Rc<RefCell<TransportCache>>,
}

/// All wiring inputs for [wire_transport_events]. Grouped to keep the call site narrow and
/// to keep ownership / cloning explicit at the boundary.
struct TransportSetup {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    idle_inhib: Rc<RefCell<Option<u32>>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    bar_show: Rc<Cell<bool>>,
    widgets: TransportWidgets,
}

fn wire_transport_events(s: TransportSetup) {
    let recent_visible = Rc::new(Cell::new(s.recent.is_visible()));
    {
        let rv = Rc::clone(&recent_visible);
        s.recent
            .connect_notify_local(Some("visible"), move |w, _| rv.set(w.is_visible()));
    }
    let ctx = Rc::new(TransportCtx {
        player: s.player.clone(),
        widgets: s.widgets,
        eof: TransportEofCtx {
            app: s.app,
            sub_pref: s.sub_pref,
            win: s.win,
            gl: s.gl,
            recent: s.recent,
            last_path: s.last_path,
            sibling_seof: s.sibling_seof,
            exit_after_current: s.exit_after_current,
            win_aspect: Rc::clone(&s.win_aspect),
            idle_inhib: s.idle_inhib,
            on_video_chrome: s.on_video_chrome,
            on_file_loaded: s.on_file_loaded,
            reapply_60: s.reapply_60,
        },
        bar_show: s.bar_show,
        recent_visible,
        sibling_nav: s.sibling_nav,
        idle_resync_pending: Rc::new(Cell::new(false)),
        tick: Rc::new(RefCell::new(None)),
        cache: Rc::new(RefCell::new(TransportCache::default())),
    });

    let ctx_drain = Rc::clone(&ctx);
    TRANSPORT_DRAIN.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || {
            drain_into_main(&ctx_drain);
        }));
    });

    if !install_observers_when_ready(&ctx) {
        let ctx2 = ctx.clone();
        TRANSPORT_INSTALL.with(|s| {
            *s.borrow_mut() = Some(Box::new(move || {
                install_observers_when_ready(&ctx2);
            }));
        });
    }
}

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

/// Drain libmpv events into [dispatch_event] immediately. Safe no-op before the GL realize hook runs.
fn transport_drain_after_loadfile() {
    TRANSPORT_DRAIN.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
}

/// Called from `wire_mpv_realize` right after the mpv bundle is created, so transport-event
/// observers attach without polling. No-op if observers were already installed.
fn trigger_transport_install() {
    let cb = TRANSPORT_INSTALL.with(|s| s.borrow_mut().take());
    if let Some(cb) = cb {
        cb();
    }
}

/// Returns true once the bundle exists and observers are installed.
fn install_observers_when_ready(ctx: &Rc<TransportCtx>) -> bool {
    let trace = std::env::var_os("RHINO_TRANSPORT_TRACE").is_some();
    let mut g = match ctx.player.try_borrow_mut() {
        Ok(g) => g,
        Err(_) => {
            if trace {
                eprintln!("[rhino] transport install: player busy, deferring");
            }
            return false;
        }
    };
    let Some(b) = g.as_mut() else {
        if trace {
            eprintln!("[rhino] transport install: player not ready, deferring");
        }
        return false;
    };
    if let Err(e) = b.observe_props(&[
        (PROP_PAUSE, "pause", Format::Flag),
        (PROP_DURATION, "duration", Format::Double),
        (PROP_VOLUME, "volume", Format::Double),
        (PROP_MUTE, "mute", Format::Flag),
        (PROP_VOLUME_MAX, "volume-max", Format::Double),
        (PROP_PATH, "path", Format::String),
    ]) {
        eprintln!("[rhino] transport observe_props failed: {e}");
        return false;
    }
    let drain_ctx = ctx.clone();
    b.install_event_drain(move || drain_into_main(&drain_ctx));
    if trace {
        eprintln!("[rhino] transport install: observers wired, draining initial events");
    }
    drop(g);
    // Pull current state directly from mpv so the play / seek / nav UI is correct **right now**,
    // even if the warm-preloaded file finished loading before observers were registered.
    transport_tick(ctx);
    refresh_sibling_nav(ctx);
    drain_into_main(ctx);
    install_transport_tick(ctx);
    true
}

fn drain_into_main(ctx: &Rc<TransportCtx>) {
    let evs = collect_events(&ctx.player);
    for e in evs {
        dispatch_event(ctx, e);
    }
}

fn collect_events(player: &Rc<RefCell<Option<MpvBundle>>>) -> Vec<TransportEv> {
    let mut out: Vec<TransportEv> = Vec::new();
    let mut g = match player.try_borrow_mut() {
        Ok(g) => g,
        Err(_) => return out,
    };
    let Some(b) = g.as_mut() else {
        return out;
    };
    b.drain_events(|ev| match ev {
        Event::PropertyChange {
            reply_userdata, change, ..
        } => {
            if let Some(t) = property_event(reply_userdata, change) {
                out.push(t);
            }
        }
        Event::FileLoaded => out.push(TransportEv::FileLoaded),
        Event::VideoReconfig => out.push(TransportEv::VideoReconfig),
        _ => {}
    });
    out
}

fn property_event(id: u64, data: PropertyData<'_>) -> Option<TransportEv> {
    Some(match (id, &data) {
        (PROP_PAUSE, PropertyData::Flag(v)) => TransportEv::Pause(*v),
        (PROP_DURATION, PropertyData::Double(v)) => TransportEv::Duration(*v),
        (PROP_VOLUME, PropertyData::Double(v)) => TransportEv::Volume(*v),
        (PROP_MUTE, PropertyData::Flag(v)) => TransportEv::Mute(*v),
        (PROP_VOLUME_MAX, PropertyData::Double(v)) => TransportEv::VolumeMax(*v),
        (PROP_PATH, PropertyData::Str(_)) => TransportEv::PathChanged,
        _ => return None,
    })
}
