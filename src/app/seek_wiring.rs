/// Wires the bottom seek bar.
///
/// `gtk::Scale` has its own `GestureDrag` on the slider; when the user grabs the thumb
/// the Scale **claims** the event sequence, which denies any extra gesture we add and
/// silently swallows its `drag-begin` / `drag-end`. A `gtk::EventControllerLegacy` sees
/// raw press / release events regardless of gesture claims, so it is the only reliable
/// way to know when the user is interacting with the slider.
///
/// Lifecycle:
/// - **press** (button or touch) → set [TransportWidgets::seek_grabbed] = true. The chrome
///   auto-hide keeps the bottom bar visible and the 1 Hz transport tick stops writing
///   positions back into the slider.
/// - **first `value-changed`** while pressed → enter drag mode: snapshot mpv `pause` and
///   the VapourSynth filter, **pause** mpv, **clear** the filter (`mvtools` / Smooth 60
///   stalls long drags otherwise), seek to the new position. Subsequent `value-changed`
///   issue a fresh `seek` on every motion so the paused frame follows the thumb live —
///   cheap because VapourSynth is unloaded.
/// - **release** → if in drag mode, commit a final `seek`, reapply the VapourSynth filter
///   when it was active, and restore the prior pause state. If only the click was a
///   trough click (no drag mode entered), no extra work is needed.
///
/// There is no watchdog: the user can hold the thumb as long as they want.
/// `EventControllerLegacy` always delivers the matching release event.

#[derive(Default)]
struct DragSnapshot {
    was_paused: Cell<bool>,
    had_vs: Cell<bool>,
    in_drag: Cell<bool>,
}

/// Shared widgets and state passed to all seek-bar event handlers in this module.
struct SeekCtx {
    seek: gtk::Scale,
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    vp: Rc<RefCell<db::VideoPrefs>>,
    snap: Rc<DragSnapshot>,
}

fn wire_seek_control(
    seek: &gtk::Scale,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    time_left: gtk::Label,
    vp: Rc<RefCell<db::VideoPrefs>>,
) {
    let ctx = Rc::new(SeekCtx {
        seek: seek.clone(),
        player: player.clone(),
        gl: gl.clone(),
        seek_sync,
        seek_grabbed,
        time_left,
        vp,
        snap: Rc::new(DragSnapshot::default()),
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
            quick_seek(&c.player, v);
            return;
        }
        if !c.snap.in_drag.replace(true) {
            drag_begin(&c.player, &c.snap, v);
        } else {
            quick_seek(&c.player, v);
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
            gtk::gdk::EventType::ButtonPress | gtk::gdk::EventType::TouchBegin => {
                c.seek_grabbed.set(true);
            }
            gtk::gdk::EventType::ButtonRelease
            | gtk::gdk::EventType::TouchEnd
            | gtk::gdk::EventType::TouchCancel => {
                c.seek_grabbed.set(false);
                if c.snap.in_drag.replace(false) {
                    drag_end(&c);
                }
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    ctx.seek.add_controller(leg);
}

fn quick_seek(player: &Rc<RefCell<Option<MpvBundle>>>, v: f64) {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return;
    };
    let s = format!("{v:.4}");
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
}

fn drag_begin(player: &Rc<RefCell<Option<MpvBundle>>>, snap: &DragSnapshot, v: f64) {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return;
    };
    snap.was_paused
        .set(b.mpv.get_property::<bool>("pause").unwrap_or(false));
    snap.had_vs.set(video_pref::has_vapoursynth_vf(&b.mpv));
    let _ = b.mpv.set_property("pause", true);
    let _ = video_pref::clear_vapoursynth_for_paused_seek(&b.mpv);
    let s = format!("{v:.4}");
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
}

fn drag_end(ctx: &SeekCtx) {
    let g = ctx.player.borrow();
    let Some(b) = g.as_ref() else {
        return;
    };
    let v = ctx.seek.value();
    let s = format!("{v:.4}");
    let _ = b.mpv.command("seek", &[s.as_str(), "absolute+keyframes"]);
    if ctx.snap.had_vs.get() {
        let mut prefs = ctx.vp.borrow_mut();
        let _ = video_pref::apply_mpv_video(&b.mpv, &mut prefs, None);
    }
    let _ = b.mpv.set_property("pause", ctx.snap.was_paused.get());
    ctx.gl.queue_render();
}
