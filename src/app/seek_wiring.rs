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
struct SeekCtx {
    seek: gtk::Scale,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
}

fn wire_seek_control(
    seek: &gtk::Scale,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
) {
    let ctx = Rc::new(SeekCtx {
        seek: seek.clone(),
        player: player.clone(),
        gl: gl.clone(),
        seek_sync,
        seek_grabbed,
        time_left,
        preview_hover_t,
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
        }
        c.gl.queue_render();
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
    ctx.gl.queue_render();
}

fn quick_seek(ctx: &SeekCtx, v: f64) {
    let g = ctx.player.borrow();
    let Some(b) = g.as_ref() else {
        return;
    };
    let s = format!("{v:.4}");
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
}
