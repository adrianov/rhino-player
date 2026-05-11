// Transport / volume / mute / EOF wiring.
//
// Property observation is used for state that changes on user/UI action (pause, duration, volume,
// mute, volume-max, path, **container-fps**) so the UI updates immediately. **`container-fps`**
// triggers a deferred Smooth / VapourSynth resync when the cadence becomes known after `loadfile`.
// Time-pos, core-idle, eof-reached, and speed are sampled by [transport_tick] every second instead — libmpv property-change events for
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
const PROP_CONTAINER_FPS: u64 = 7;

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
    /// `container-fps` changed — refresh `RHINO_SOURCE_FPS` / `.vpy` graph after prev/next `loadfile`.
    ContainerFpsChanged,
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
    speed_readout: gtk::Label,
    vol_menu: gtk::MenuButton,
    vol_header_img: gtk::Image,
    vol_readout: gtk::Label,
    vol_adj: gtk::Adjustment,
    vol_mute: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
    sub_readout: gtk::Label,
    smooth_toolbar_status: gtk::Label,
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
    recent: gtk::Box,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    playback_focus: Rc<Cell<bool>>,
}

struct TransportCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    widgets: TransportWidgets,
    eof: TransportEofCtx,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    smooth_budget_decoder: Rc<RefCell<crate::video_pref::SmoothBudgetDecoderState>>,
    /// Bottom-bar visibility flag; transient seek-slider redraws are skipped while it is `false`
    /// to avoid invalidating chrome that is animating in / out (the cause of fullscreen flicker).
    bar_show: Rc<Cell<bool>>,
    /// Toggled to keep the recent grid path in sync; if `recent` is visible the seek bar is hidden too.
    recent_visible: Rc<Cell<bool>>,
    sibling_nav: SiblingNavUi,
    /// Coalesce [glib::idle_add_local_once] resyncs on `FileLoaded` / `path` churn.
    idle_resync_pending: Rc<Cell<bool>>,
    /// Debounced [glib::timeout_add_local] after `FileLoaded` / `VideoReconfig` / `path` / `container-fps`
    /// so one [smooth_60_full_resync_after_media_change] runs when the burst settles.
    smooth_60_resync_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    /// 1 Hz timer source id (kept so it can be replaced if observers re-install).
    tick: Rc<RefCell<Option<glib::SourceId>>>,
    cache: Rc<RefCell<TransportCache>>,
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
}

/// All wiring inputs for [wire_transport_events]. Grouped to keep the call site narrow and
/// to keep ownership / cloning explicit at the boundary.
struct TransportSetup {
    app: adw::Application,
    player: Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    sub_pref: Rc<RefCell<db::SubPrefs>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
    recent: gtk::Box,
    /// Shared with [BackToBrowseCtx]; refreshed before pausing when returning to the continue list.
    recent_visible: Rc<Cell<bool>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    sibling_nav: SiblingNavUi,
    exit_after_current: Rc<Cell<bool>>,
    win_aspect: Rc<Cell<Option<f64>>>,
    idle_inhib: Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    mpv_teardown_after_draw: Rc<Cell<bool>>,
    on_video_chrome: Rc<dyn Fn()>,
    on_file_loaded: Rc<dyn Fn()>,
    reapply_60: VideoReapply60,
    hdr_title_mirror: Option<Rc<gtk::Label>>,
    bar_show: Rc<Cell<bool>>,
    playback_focus: Rc<Cell<bool>>,
    widgets: TransportWidgets,
    seek_chapters: Rc<RefCell<Vec<(f64, String)>>>,
}

fn wire_transport_events(s: TransportSetup) {
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
            mpv_teardown_after_draw: s.mpv_teardown_after_draw,
            on_video_chrome: s.on_video_chrome,
            on_file_loaded: s.on_file_loaded,
            reapply_60: s.reapply_60,
            hdr_title_mirror: s.hdr_title_mirror.clone(),
            playback_focus: Rc::clone(&s.playback_focus),
        },
        video_pref: s.video_pref.clone(),
        smooth_budget_decoder: Rc::new(RefCell::new(
            crate::video_pref::SmoothBudgetDecoderState::default(),
        )),
        bar_show: s.bar_show,
        recent_visible: s.recent_visible,
        sibling_nav: s.sibling_nav,
        idle_resync_pending: Rc::new(Cell::new(false)),
        smooth_60_resync_debounce: Rc::new(RefCell::new(None)),
        tick: Rc::new(RefCell::new(None)),
        cache: Rc::new(RefCell::new(TransportCache::default())),
        seek_chapters: s.seek_chapters.clone(),
    });

    let ctx_drain = Rc::clone(&ctx);
    TRANSPORT_DRAIN.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || {
            drain_into_main(&ctx_drain);
        }));
    });

    let ctx_smooth = Rc::clone(&ctx);
    REQUEST_SMOOTH_60_RESYNC.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || {
            schedule_smooth_60_resync_idle(&ctx_smooth);
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

thread_local! {
    /// Set by [wire_transport_events]. Seek / keyframe tails and **unpause** schedule the same debounced
    /// [schedule_smooth_60_resync_idle] as `FileLoaded` so Smooth is not applied twice in one interaction.
    static REQUEST_SMOOTH_60_RESYNC: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Coalesce Smooth 60 / VapourSynth rebuild with transport (same timer as `FileLoaded` / `path` churn).
fn request_smooth_60_transport_resync() {
    REQUEST_SMOOTH_60_RESYNC.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    });
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

include!("transport_observe_install.rs");
