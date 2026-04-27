// Event-driven transport / volume / mute / EOF wiring (replaces a 200ms poll).
//
// All UI mirrors of mpv state are driven by `mpv_observe_property` + `mpv_set_wakeup_callback`,
// drained on the GTK main thread. See `.cursor/rules/events-over-polling.mdc` and
// `docs/features/03-mpv-embedding.md`.
//
// The only periodic timer left is a short tail-stall watcher that runs while playback is
// within ~1.75 s of the end (libmpv's `keep-open` can leave `eof-reached==false` for ~1 s near
// the tail; see `docs/features/07-sibling-folder-queue.md`).
//
// Diagnostics: set `RHINO_TRANSPORT_TRACE=1` in the environment to print every dispatched event
// to stderr — useful when transport UI (play button icon, seek slider) appears stuck, to confirm
// whether mpv is delivering `Pause` / `Duration` events or the issue is somewhere else.

const PROP_PAUSE: u64 = 1;
const PROP_TIME_POS: u64 = 2;
const PROP_DURATION: u64 = 3;
const PROP_VOLUME: u64 = 4;
const PROP_MUTE: u64 = 5;
const PROP_VOLUME_MAX: u64 = 6;
const PROP_EOF_REACHED: u64 = 7;
const PROP_PATH: u64 = 8;

const TAIL_STALL_INTERVAL: Duration = Duration::from_millis(200);
/// Lower bound between two `time-pos` driven seek-slider redraws. mpv emits `time-pos`
/// every video frame (24–60 Hz), but adjacent chrome (tooltips, hover popovers) can flicker
/// when the seek `GtkScale` repaints that often. 10 Hz is smooth enough for seek feedback.
const TIME_POS_MIN_GAP: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
enum TransportEv {
    Pause(bool),
    TimePos(f64),
    Duration(f64),
    Volume(f64),
    Mute(bool),
    VolumeMax(f64),
    EofReached(bool),
    EndFile,
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
    time_left: gtk::Label,
    time_right: gtk::Label,
    speed_menu: gtk::MenuButton,
    vol_menu: gtk::MenuButton,
    vol_adj: gtk::Adjustment,
    vol_mute: gtk::ToggleButton,
    vol_sync: Rc<Cell<bool>>,
}

struct TransportCache {
    duration: f64,
    pause: bool,
    eof: bool,
    pos: f64,
    last_pos_apply: Option<Instant>,
}

impl Default for TransportCache {
    fn default() -> Self {
        Self {
            duration: 0.0,
            pause: false,
            eof: false,
            pos: 0.0,
            last_pos_apply: None,
        }
    }
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
    tail_timer: Rc<RefCell<Option<glib::SourceId>>>,
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
        tail_timer: Rc::new(RefCell::new(None)),
        cache: Rc::new(RefCell::new(TransportCache::default())),
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
        (PROP_TIME_POS, "time-pos", Format::Double),
        (PROP_DURATION, "duration", Format::Double),
        (PROP_VOLUME, "volume", Format::Double),
        (PROP_MUTE, "mute", Format::Flag),
        (PROP_VOLUME_MAX, "volume-max", Format::Double),
        (PROP_EOF_REACHED, "eof-reached", Format::Flag),
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
    // Initial property values are emitted asynchronously by libmpv; pull current state
    // directly from mpv so the play / seek / nav UI is correct **right now**, even if
    // the warm-preloaded file finished loading before observers were registered.
    resync_play_button(ctx);
    refresh_sibling_nav(ctx);
    drain_into_main(ctx);
    true
}

fn drain_into_main(ctx: &Rc<TransportCtx>) {
    let evs = collect_events(&ctx.player);
    for e in evs {
        dispatch_event(ctx, e);
    }
    update_tail_timer(ctx);
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
        Event::EndFile(_) => out.push(TransportEv::EndFile),
        Event::FileLoaded => out.push(TransportEv::FileLoaded),
        Event::VideoReconfig => out.push(TransportEv::VideoReconfig),
        _ => {}
    });
    out
}

fn property_event(id: u64, data: PropertyData<'_>) -> Option<TransportEv> {
    Some(match (id, &data) {
        (PROP_PAUSE, PropertyData::Flag(v)) => TransportEv::Pause(*v),
        (PROP_TIME_POS, PropertyData::Double(v)) => TransportEv::TimePos(*v),
        (PROP_DURATION, PropertyData::Double(v)) => TransportEv::Duration(*v),
        (PROP_VOLUME, PropertyData::Double(v)) => TransportEv::Volume(*v),
        (PROP_MUTE, PropertyData::Flag(v)) => TransportEv::Mute(*v),
        (PROP_VOLUME_MAX, PropertyData::Double(v)) => TransportEv::VolumeMax(*v),
        (PROP_EOF_REACHED, PropertyData::Flag(v)) => TransportEv::EofReached(*v),
        (PROP_PATH, PropertyData::Str(_)) => TransportEv::PathChanged,
        _ => return None,
    })
}

