/// Bottom seek bar wiring.
///
/// **Release** seeks to **`preview_hover_t`**: **`cursor_x / bar_width × duration`**, same formula as
/// **`seek_bar_preview`** (label + thumbnail scrubber).
///
/// While **`seek_grabbed`** (pointer down), thumb or trough **`value_changed`** adjusts the scale and
/// elapsed label locally; **`quick_seek`** runs on **release** only. When **not** grabbed,
/// **`value_changed`** still **`quick_seek`** (keyboard/scroll or other inputs that skip the latch).
///
/// Motion over the bar always updates **`hover_t`**. When hover preview is off, **`seek_bar_preview`**
/// skips **`loadfile`** / GL **only** — **`hover_t`** still tracks the pointer.
struct SeekControlDeps {
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
    reapply_60: VideoReapply60,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
}

struct SeekCtx {
    seek: gtk::Scale,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
    reapply_60: VideoReapply60,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
}

fn wire_seek_control(seek: &gtk::Scale, d: SeekControlDeps) {
    let SeekControlDeps {
        player,
        gl,
        seek_sync,
        seek_grabbed,
        time_left,
        preview_hover_t,
        reapply_60,
        smooth_seek_debounce,
        resume_after_seek_idle,
        play_toggle,
    } = d;
    let ctx = Rc::new(SeekCtx {
        seek: seek.clone(),
        player,
        gl,
        seek_sync,
        seek_grabbed,
        time_left,
        preview_hover_t,
        reapply_60,
        smooth_seek_debounce,
        resume_after_seek_idle,
        play_toggle,
    });
    wire_value_changed(&ctx);
    wire_press_release(&ctx);
}

fn wire_value_changed(ctx: &Rc<SeekCtx>) {
    let c = Rc::clone(ctx);
    ctx.seek.connect_value_changed(move |r| {
        if c.seek_sync.get() {
            return;
        }
        let v = r.value();
        let s = format_time(v);
        if c.time_left.text().as_str() != s {
            c.time_left.set_text(&s);
        }
        if !c.seek_grabbed.get() {
            quick_seek(&c, v);
        } else {
            c.gl.queue_render();
        }
    });
}

fn wire_press_release(ctx: &Rc<SeekCtx>) {
    let leg = gtk::EventControllerLegacy::new();
    leg.set_propagation_phase(gtk::PropagationPhase::Capture);
    let c = Rc::clone(ctx);
    leg.connect_event(move |_, ev| {
        match ev.event_type() {
            gtk::gdk::EventType::ButtonPress => {
                if let Some(be) = ev.downcast_ref::<gtk::gdk::ButtonEvent>() {
                    if be.button() != gtk::gdk::BUTTON_PRIMARY {
                        return glib::Propagation::Proceed;
                    }
                }
                c.seek_grabbed.set(true);
            }
            gtk::gdk::EventType::TouchBegin => {
                c.seek_grabbed.set(true);
            }
            gtk::gdk::EventType::ButtonRelease
            | gtk::gdk::EventType::TouchEnd
            | gtk::gdk::EventType::TouchCancel => {
                if !c.seek_grabbed.get() {
                    return glib::Propagation::Proceed;
                }
                c.seek_grabbed.set(false);
                commit_preview_seek(&c);
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    ctx.seek.add_controller(leg);
}

fn commit_preview_seek(ctx: &SeekCtx) {
    let upper = ctx.seek.adjustment().upper();
    if upper <= 0.0 || !upper.is_finite() {
        ctx.gl.queue_render();
        return;
    }
    let t = ctx.preview_hover_t.get().clamp(0.0, upper);
    ctx.seek_sync.set(true);
    ctx.seek.set_value(t);
    ctx.seek_sync.set(false);
    quick_seek(ctx, t);
}

/// Idle after the last seek in a burst: unpause if playback was running before the burst, then reattach Smooth when due.
const SEEK_BURST_TAIL_IDLE_MS: u64 = 1000;

#[derive(Clone, Copy)]
enum SeekKeyframeKind {
    /// Pause-if-playing before seek; after idle, unpause only if the burst began while playing (arrow keys).
    ArrowBurst,
    /// Do not change pause state; debounce Smooth reattach when the seek starts while playing (seek bar, MPRIS).
    ScaleOrExternal,
}

struct SeekKeyframeParams<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    gl: &'a gtk::GLArea,
    reapply_60: &'a VideoReapply60,
    smooth_seek_debounce: &'a Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: &'a Rc<Cell<bool>>,
    play_toggle: &'a PlayToggleCtx,
}

fn cancel_smooth_seek_debounce(slot: &Rc<RefCell<Option<glib::SourceId>>>) {
    if let Some(id) = slot.borrow_mut().take() {
        id.remove();
    }
}

fn schedule_smooth_vf_only_tail(
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    reapply_60: VideoReapply60,
) {
    cancel_smooth_seek_debounce(slot);
    let deb = Rc::clone(slot);
    let id = glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
        *deb.borrow_mut() = None;
        smooth_vf_attach_if_playing(player, gl, reapply_60, false);
    });
    *slot.borrow_mut() = Some(id);
}

fn schedule_seek_burst_tail(
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    reapply_60: VideoReapply60,
    play_toggle: PlayToggleCtx,
) {
    cancel_smooth_seek_debounce(slot);
    let deb = Rc::clone(slot);
    let id = glib::timeout_add_local_once(Duration::from_millis(SEEK_BURST_TAIL_IDLE_MS), move || {
        *deb.borrow_mut() = None;
        let trust_unpause = resume_after_seek_idle.replace(false);
        if trust_unpause {
            let _ = apply_mpv_pause(&play_toggle, false);
        }
        smooth_vf_attach_if_playing(player, gl, reapply_60, trust_unpause);
    });
    *slot.borrow_mut() = Some(id);
}

/// Seek main mpv with `absolute+keyframes`. Drops vapoursynth **`vf`** before the seek when still
/// present.
///
/// **[SeekKeyframeKind::ArrowBurst]**: pause through **`apply_mpv_pause`** when the clip was
/// playing; remember “should resume” for the whole burst; after [`SEEK_BURST_TAIL_IDLE_MS`] without
/// another seek, unpause if so and reattach Smooth — coalesces rapid arrow seeks.
///
/// **[SeekKeyframeKind::ScaleOrExternal]**: leaves pause alone; if this seek begins while playing,
/// debounce Smooth reattach only. If an arrow burst left **`resume_after_seek_idle`** latched, the
/// same tail timer still runs (seek-bar scrub while “held” paused for arrows).
fn main_player_seek_keyframes(p: &SeekKeyframeParams<'_>, kind: SeekKeyframeKind, seconds: &str) {
    cancel_smooth_seek_debounce(p.smooth_seek_debounce);
    let paused_before;
    {
        let g = p.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        paused_before = b.mpv.get_property::<bool>("pause").unwrap_or(true);
    }
    if matches!(kind, SeekKeyframeKind::ArrowBurst) {
        let was_playing = !paused_before;
        p.resume_after_seek_idle
            .set(p.resume_after_seek_idle.get() || was_playing);
        if was_playing {
            let _ = apply_mpv_pause(p.play_toggle, true);
        }
    }
    {
        let g = p.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        let _ = video_pref::unload_smooth_on_pause(&b.mpv);
        let _ = b.mpv.command("seek", &[seconds, "absolute+keyframes"]);
    }
    if p.resume_after_seek_idle.get() {
        schedule_seek_burst_tail(
            p.smooth_seek_debounce,
            p.resume_after_seek_idle.clone(),
            p.player.clone(),
            p.gl.clone(),
            p.reapply_60.clone(),
            p.play_toggle.clone(),
        );
    } else if matches!(kind, SeekKeyframeKind::ScaleOrExternal) && !paused_before {
        schedule_smooth_vf_only_tail(
            p.smooth_seek_debounce,
            p.player.clone(),
            p.gl.clone(),
            p.reapply_60.clone(),
        );
    }
    p.gl.queue_render();
}

fn quick_seek(ctx: &SeekCtx, v: f64) {
    let s = format!("{v:.4}");
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: &ctx.player,
            gl: &ctx.gl,
            reapply_60: &ctx.reapply_60,
            smooth_seek_debounce: &ctx.smooth_seek_debounce,
            resume_after_seek_idle: &ctx.resume_after_seek_idle,
            play_toggle: &ctx.play_toggle,
        },
        SeekKeyframeKind::ScaleOrExternal,
        &s,
    );
}

struct SeekArrowDeps<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    seek: &'a gtk::Scale,
    seek_sync: &'a Rc<Cell<bool>>,
    time_left: &'a gtk::Label,
    gl: &'a gtk::GLArea,
    reapply_60: &'a VideoReapply60,
    smooth_seek_debounce: &'a Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: &'a Rc<Cell<bool>>,
    play_toggle: &'a PlayToggleCtx,
}

/// Steps **playback position** by `delta_sec` (e.g. −5 / +5 for arrow keys); keeps UI scale + clock aligned.
fn seek_arrow_step(d: &SeekArrowDeps<'_>, delta_sec: f64) {
    let g = d.player.borrow();
    let Some(b) = g.as_ref() else {
        return;
    };
    let pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let adj_u = d.seek.adjustment().upper();
    let adj_u = if adj_u.is_finite() { adj_u.max(0.0) } else { 0.0 };
    let len = if adj_u > 0.0 {
        adj_u
    } else if dur > 0.0 {
        dur
    } else {
        return;
    };
    let nt = (pos + delta_sec).clamp(0.0, len);
    drop(g);
    let s_abs = format!("{nt:.4}");
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: d.player,
            gl: d.gl,
            reapply_60: d.reapply_60,
            smooth_seek_debounce: d.smooth_seek_debounce,
            resume_after_seek_idle: d.resume_after_seek_idle,
            play_toggle: d.play_toggle,
        },
        SeekKeyframeKind::ArrowBurst,
        &s_abs,
    );
    d.seek_sync.set(true);
    d.seek.set_value(nt);
    d.seek_sync.set(false);
    let ts = format_time(nt);
    if d.time_left.text().as_str() != ts {
        d.time_left.set_text(&ts);
    }
}
