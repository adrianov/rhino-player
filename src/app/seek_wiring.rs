/// Bottom seek bar wiring.
///
/// Trough / thumb interaction uses stock **`GtkRange`** behavior with
/// **`gtk-primary-button-warps-slider`** (see `theme::apply`, same as the volume scale).
/// While **`seek_grabbed`**, **`value_changed`** moves the thumb locally; **`quick_seek`** runs on
/// **release** to **`preview_hover_t`** (pointer / preview label time), not the raw thumb value.
/// When preview is off, release falls back to the capped thumb time. When not grabbed,
/// **`value_changed`** seeks immediately (keyboard / scroll).
struct SeekControlDeps {
    player: Rc<RefCell<Option<MpvBundle>>>,
    preview_player: Rc<RefCell<Option<crate::mpv_embed::MpvPreviewGl>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    seek_preview_on: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
}

struct SeekCtx {
    seek: gtk::Scale,
    player: Rc<RefCell<Option<MpvBundle>>>,
    preview_player: Rc<RefCell<Option<crate::mpv_embed::MpvPreviewGl>>>,
    gl: gtk::GLArea,
    seek_sync: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    seek_preview_on: Rc<Cell<bool>>,
    time_left: gtk::Label,
    preview_hover_t: Rc<Cell<f64>>,
    smooth_seek_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    resume_after_seek_idle: Rc<Cell<bool>>,
    play_toggle: PlayToggleCtx,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
}

fn release_seek_time(ctx: &SeekCtx) -> f64 {
    let upper = ctx.seek.adjustment().upper();
    let raw = ctx.seek.value();
    if ctx.seek_preview_on.get() {
        ctx.preview_hover_t.get()
    } else {
        bar_label_time_from_value(ctx, raw).unwrap_or(raw)
    }
    .clamp(0.0, upper.max(0.0))
}

fn wire_seek_control(seek: &gtk::Scale, d: SeekControlDeps) {
    let SeekControlDeps {
        player,
        preview_player,
        gl,
        seek_sync,
        seek_grabbed,
        seek_preview_on,
        time_left,
        preview_hover_t,
        smooth_seek_debounce,
        resume_after_seek_idle,
        play_toggle,
        dvd_bar,
    } = d;
    let ctx = Rc::new(SeekCtx {
        seek: seek.clone(),
        player,
        preview_player,
        gl,
        seek_sync,
        seek_grabbed,
        seek_preview_on,
        time_left,
        preview_hover_t,
        smooth_seek_debounce,
        resume_after_seek_idle,
        play_toggle,
        dvd_bar,
    });
    wire_value_changed(&ctx);
    wire_press_release(&ctx);
}

fn bar_label_time_from_value(ctx: &SeekCtx, value: f64) -> Option<f64> {
    let upper = ctx.seek.adjustment().upper();
    let main = ctx.player.borrow();
    let shell = main
        .as_ref()
        .and_then(|b| b.me_budget_shell_path.borrow().clone());
    let preview = ctx.preview_player.borrow();
    crate::seek_bar_preview::seek_bar_label_time_from_value(
        upper,
        value,
        main.as_ref().map(|b| &b.mpv),
        shell.as_deref(),
        preview.as_ref().map(|p| &p.mpv),
        Some(&ctx.dvd_bar),
    )
}

fn wire_value_changed(ctx: &Rc<SeekCtx>) {
    let c = Rc::clone(ctx);
    ctx.seek.connect_value_changed(move |r| {
        if c.seek_sync.get() {
            return;
        }
        let v = r.value();
        let t = bar_label_time_from_value(&c, v).unwrap_or(v);
        if c.seek_grabbed.get() {
            let label_t = if c.seek_preview_on.get() {
                c.preview_hover_t.get()
            } else {
                t
            };
            sync_time_left_label(&c.time_left, label_t);
            c.gl.queue_render();
            return;
        }
        sync_time_left_label(&c.time_left, t);
        quick_seek(&c, t);
    });
}

fn sync_time_left_label(time_left: &gtk::Label, t: f64) {
    let s = format_time(t);
    if time_left.text().as_str() != s {
        time_left.set_text(&s);
    }
}

fn wire_press_release(ctx: &Rc<SeekCtx>) {
    let leg = gtk::EventControllerLegacy::new();
    // Capture: latch grab before GtkRange warp `value_changed` (defer seek until release).
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
    let t = release_seek_time(ctx);
    crate::user_action_log::act(format!("seek bar release -> t={t:.2}s"));
    ctx.seek_sync.set(true);
    ctx.seek.set_value(t);
    ctx.seek_sync.set(false);
    quick_seek(ctx, t);
    ctx.gl.queue_render();
}

include!("seek_wiring/seek_keyframes.rs");
include!("seek_wiring/seek_arrows.rs");
