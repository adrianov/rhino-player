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
pub(crate) const TICK_EOF_TAIL_SEC: f64 = 1.5;

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
    /// mpv `EndFile` with error reason (unrecognized / demux failure after async `loadfile`).
    LoadFailed,
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
    win_aspect: Rc<WinAspectCell>,
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
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    blackout: Rc<crate::screen_blackout::BlackoutSync>,
    continue_grid_cache: crate::media_probe::ContinueGridCache,
    on_open_fail: Rc<dyn Fn(String)>,
}

fn wire_transport_events(s: TransportSetup) {
    let ctx = build_transport_ctx(s);

    let ctx_drain = Rc::clone(&ctx);
    TRANSPORT_DRAIN.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || {
            drain_into_main(&ctx_drain);
        }));
    });
    register_transport_tick_slot(&ctx);
    register_warm_browse_slot(&ctx);
    register_smooth_resync_slots(&ctx);
    arm_transport_install(&ctx);
}

fn register_transport_tick_slot(ctx: &Rc<TransportCtx>) {
    let ctx_tick = Rc::clone(ctx);
    TRANSPORT_TICK.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || {
            transport_tick(&ctx_tick);
            schedule_transport_resync_on_idle(&ctx_tick);
        }));
    });
}

fn register_warm_browse_slot(ctx: &Rc<TransportCtx>) {
    let ctx_browse = Rc::clone(ctx);
    WARM_BROWSE_SYNC.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move |path: PathBuf| {
            warm_browse_sync_tick(&ctx_browse, path);
        }));
    });
}

/// Continue-grid hover / warm `loadfile` start: DB resume + duration, prev/next from [last_path].
fn warm_browse_sync_tick(ctx: &Rc<TransportCtx>, path: PathBuf) {
    *ctx.eof.last_path.borrow_mut() = Some(path.clone());
    if !crate::playback_entity::PlaybackEntity::resolve(&path).uses_dvd_bar_cache() {
        *ctx.dvd_bar.borrow_mut() = None;
    }
    refresh_sibling_nav(ctx);
    transport_tick(ctx);
}

fn register_smooth_resync_slots(ctx: &Rc<TransportCtx>) {
    let ctx_smooth = Rc::clone(ctx);
    let smooth_resync: Rc<dyn Fn()> = Rc::new(move || {
        schedule_smooth_60_resync_idle(&ctx_smooth);
    });
    REQUEST_SMOOTH_60_RESYNC.with(|slot| {
        *slot.borrow_mut() = Some(Rc::clone(&smooth_resync));
    });
    crate::video_pref::register_vf_swap_smooth_resync(smooth_resync);
    register_cancel_smooth_resync_slot(ctx);
}

fn register_cancel_smooth_resync_slot(ctx: &Rc<TransportCtx>) {
    let ctx_cancel = Rc::clone(ctx);
    CANCEL_SMOOTH_60_RESYNC.with(|slot| {
        *slot.borrow_mut() = Some(Rc::new(move || cancel_smooth_60_resync_idle(&ctx_cancel)));
    });
}

fn arm_transport_install(ctx: &Rc<TransportCtx>) {
    if !install_observers_when_ready(ctx) {
        let ctx2 = ctx.clone();
        TRANSPORT_INSTALL.with(|s| {
            *s.borrow_mut() = Some(Box::new(move || {
                install_observers_when_ready(&ctx2);
            }));
        });
    }
}

include!("transport_ctx.rs");
include!("transport_ctx_build.rs");
include!("transport_slots.rs");
include!("transport_observe_install.rs");
