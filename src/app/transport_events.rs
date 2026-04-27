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

fn dispatch_event(ctx: &Rc<TransportCtx>, ev: TransportEv) {
    let w = &ctx.widgets;
    if std::env::var_os("RHINO_TRANSPORT_TRACE").is_some() {
        eprintln!("[rhino] transport ev: {ev:?}");
    }
    match ev {
        TransportEv::Pause(p) => {
            ctx.cache.borrow_mut().pause = p;
            sync_play_button(w, ctx.cache.borrow().duration, p);
        }
        TransportEv::Duration(d) => {
            let d = if d.is_finite() { d } else { 0.0 };
            ctx.cache.borrow_mut().duration = d;
            sync_seek_range(w, d);
            sync_play_button(w, d, ctx.cache.borrow().pause);
            sync_speed_button(w, d);
        }
        TransportEv::TimePos(p) => apply_time_pos(ctx, p),
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::EofReached(eof) => {
            ctx.cache.borrow_mut().eof = eof;
            if eof {
                run_sibling_eof(ctx);
            }
        }
        TransportEv::EndFile => run_sibling_eof(ctx),
        TransportEv::FileLoaded | TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            resync_play_button(ctx);
        }
        TransportEv::PathChanged => {
            refresh_sibling_nav(ctx);
            resync_play_button(ctx);
        }
    }
}

/// On `FileLoaded` / `VideoReconfig`, mpv may have already emitted the new `pause` /
/// `duration` values before the observer was installed (warm preload), or the events
/// may have been coalesced. Re-read both properties straight from mpv so the play
/// button always reflects the actual playback state without waiting for the next event.
fn resync_play_button(ctx: &Rc<TransportCtx>) {
    let g = match ctx.player.try_borrow() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let dur = if dur.is_finite() { dur } else { 0.0 };
    {
        let mut c = ctx.cache.borrow_mut();
        c.pause = pause;
        c.duration = dur;
    }
    sync_play_button(&ctx.widgets, dur, pause);
    sync_speed_button(&ctx.widgets, dur);
    sync_seek_range(&ctx.widgets, dur);
}

/// Recomputes Prev/Next sensitivity + tooltips. Called on `path`/`FileLoaded`/`VideoReconfig`
/// instead of the previous 200ms poll, so the bottom-bar nav always reflects the loaded file.
fn refresh_sibling_nav(ctx: &Rc<TransportCtx>) {
    let cur = current_local_path(&ctx.player).or_else(|| ctx.eof.last_path.borrow().clone());
    ctx.sibling_nav
        .refresh(cur.as_deref(), ctx.eof.sibling_seof.as_ref());
}

fn current_local_path(player: &Rc<RefCell<Option<MpvBundle>>>) -> Option<PathBuf> {
    let g = player.try_borrow().ok()?;
    let b = g.as_ref()?;
    local_file_from_mpv(&b.mpv)
}

fn apply_time_pos(ctx: &Rc<TransportCtx>, p: f64) {
    let dur = {
        let mut c = ctx.cache.borrow_mut();
        c.pos = p;
        c.duration
    };
    let bar_visible = ctx.bar_show.get() || ctx.recent_visible.get();
    if bar_visible {
        update_time_labels(&ctx.widgets, p, dur);
    }
    let now = Instant::now();
    let allow = {
        let c = ctx.cache.borrow();
        c.last_pos_apply
            .map(|t| now.duration_since(t) >= TIME_POS_MIN_GAP)
            .unwrap_or(true)
    };
    if !allow || !bar_visible {
        return;
    }
    ctx.cache.borrow_mut().last_pos_apply = Some(now);
    sync_seek_pos(&ctx.widgets, p, dur);
}

fn sync_window_aspect_from_player(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win_aspect: &Rc<Cell<Option<f64>>>,
) {
    let g = match player.try_borrow() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(b) = g.as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, win_aspect.as_ref());
    }
}

fn run_sibling_eof(ctx: &Rc<TransportCtx>) {
    let e = &ctx.eof;
    maybe_advance_sibling_on_eof(
        &ctx.player,
        &e.win,
        &e.gl,
        &e.recent,
        &e.last_path,
        e.sibling_seof.as_ref(),
        &e.exit_after_current,
        &e.app,
        &e.sub_pref,
        &e.idle_inhib,
        &e.on_video_chrome,
        Rc::clone(&e.win_aspect),
        Some(Rc::clone(&e.on_file_loaded)),
        &e.reapply_60,
    );
}

fn update_tail_timer(ctx: &Rc<TransportCtx>) {
    let cache = ctx.cache.borrow();
    let needs = !cache.pause
        && cache.duration > 0.0
        && cache.pos.is_finite()
        && (cache.duration - cache.pos) <= SIBLING_END_SLACK_SEC;
    drop(cache);
    let mut slot = ctx.tail_timer.borrow_mut();
    if needs {
        if slot.is_some() {
            return;
        }
        let ctx_t = ctx.clone();
        let id = glib::timeout_add_local(TAIL_STALL_INTERVAL, move || {
            run_sibling_eof(&ctx_t);
            let c = ctx_t.cache.borrow();
            let still = !c.pause
                && c.duration > 0.0
                && c.pos.is_finite()
                && (c.duration - c.pos) <= SIBLING_END_SLACK_SEC;
            if still {
                glib::ControlFlow::Continue
            } else {
                *ctx_t.tail_timer.borrow_mut() = None;
                glib::ControlFlow::Break
            }
        });
        *slot = Some(id);
    } else if let Some(id) = slot.take() {
        id.remove();
    }
}

fn sync_play_button(w: &TransportWidgets, dur: f64, paused: bool) {
    let has_media = dur > 0.0;
    if w.play_pause.is_sensitive() != has_media {
        w.play_pause.set_sensitive(has_media);
    }
    let (icon, tip) = if has_media && !paused {
        ("media-playback-pause-symbolic", "Pause (Space)")
    } else if has_media {
        ("media-playback-start-symbolic", "Play (Space)")
    } else {
        ("media-playback-start-symbolic", "No media")
    };
    if w.play_pause.icon_name().as_deref() != Some(icon) {
        w.play_pause.set_icon_name(icon);
    }
    set_tooltip_if_changed(w.play_pause.upcast_ref::<gtk::Widget>(), tip);
}

fn sync_speed_button(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.speed_menu.is_sensitive() != has_media {
        w.speed_menu.set_sensitive(has_media);
    }
}

fn sync_seek_range(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.seek.is_sensitive() != has_media {
        w.seek.set_sensitive(has_media);
    }
    if has_media && (w.seek_adj.upper() - dur).abs() > f64::EPSILON {
        w.seek_adj.set_lower(0.0);
        w.seek_adj.set_upper(dur);
    }
}

fn sync_seek_pos(w: &TransportWidgets, pos: f64, dur: f64) {
    if dur <= 0.0 || !pos.is_finite() {
        return;
    }
    let v = pos.clamp(0.0, dur);
    if (w.seek_adj.value() - v).abs() < 0.01 {
        return;
    }
    w.seek_sync.set(true);
    w.seek_adj.set_value(v);
    w.seek_sync.set(false);
}

fn update_time_labels(w: &TransportWidgets, pos: f64, dur: f64) {
    let pos_s = format_time(pos);
    if w.time_left.label().as_str() != pos_s {
        w.time_left.set_label(&pos_s);
    }
    let dur_s = format_time(dur);
    if w.time_right.label().as_str() != dur_s {
        w.time_right.set_label(&dur_s);
    }
}

fn sync_volume(w: &TransportWidgets, vol: f64) {
    let muted = w.vol_mute.is_active();
    let v_icon = vol_icon(muted, vol);
    if w.vol_menu.icon_name().as_deref() != Some(v_icon) {
        w.vol_menu.set_icon_name(v_icon);
    }
    if w.vol_menu.is_active() {
        return;
    }
    let clamped = vol.clamp(0.0, w.vol_adj.upper());
    if (w.vol_adj.value() - clamped).abs() < 0.01 {
        return;
    }
    w.vol_sync.set(true);
    w.vol_adj.set_value(clamped);
    w.vol_sync.set(false);
}

fn sync_mute(w: &TransportWidgets, muted: bool) {
    let icon = vol_mute_pop_icon(muted);
    if w.vol_mute.icon_name().as_deref() != Some(icon) {
        w.vol_mute.set_icon_name(icon);
    }
    if w.vol_mute.is_active() != muted {
        w.vol_sync.set(true);
        w.vol_mute.set_active(muted);
        w.vol_sync.set(false);
    }
    set_tooltip_if_changed(
        w.vol_mute.upcast_ref::<gtk::Widget>(),
        if muted { "Unmute" } else { "Mute" },
    );
}

fn sync_volume_max(w: &TransportWidgets, vmax: f64) {
    if vmax.is_finite() && vmax > 0.0 && (w.vol_adj.upper() - vmax).abs() > f64::EPSILON {
        w.vol_adj.set_upper(vmax);
    }
}

fn set_tooltip_if_changed(w: &gtk::Widget, tip: &str) {
    if w.tooltip_text().as_deref() != Some(tip) {
        w.set_tooltip_text(Some(tip));
    }
}
