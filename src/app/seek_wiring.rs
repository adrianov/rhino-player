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

include!("seek_wiring/seek_keyframes.rs");

fn quick_seek(ctx: &SeekCtx, v: f64) {
    let s = format!("{v:.4}");
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: &ctx.player,
            gl: &ctx.gl,
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
