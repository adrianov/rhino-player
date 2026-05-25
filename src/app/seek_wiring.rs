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
            let s = format_time(label_t);
            if c.time_left.text().as_str() != s {
                c.time_left.set_text(&s);
            }
            c.gl.queue_render();
            return;
        }
        let s = format_time(t);
        if c.time_left.text().as_str() != s {
            c.time_left.set_text(&s);
        }
        quick_seek(&c, t);
    });
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
    ctx.seek_sync.set(true);
    ctx.seek.set_value(t);
    ctx.seek_sync.set(false);
    quick_seek(ctx, t);
    ctx.gl.queue_render();
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
            dvd_bar: Some(&ctx.dvd_bar),
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
    dvd_bar: Option<&'a Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
}

#[must_use]
fn dvd_title_pos(
    b: &MpvBundle,
    ch: &std::path::Path,
    local: f64,
    live: f64,
    dvd_bar: Option<&Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
) -> Option<(f64, f64)> {
    if let Some(slot) = dvd_bar {
        let guard = slot.borrow();
        if let Some(ref bar) = *guard {
            return Some((bar.transport_global_pos(b, ch, local), bar.total_sec()));
        }
    }
    crate::dvd_vob_timeline::DvdBarState::build(ch, live)
        .map(|bar| (bar.transport_global_pos(b, ch, local), bar.total_sec()))
}

fn arrow_seek_pos_len(
    b: &MpvBundle,
    seek: &gtk::Scale,
    dvd_bar: Option<&Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>>,
) -> Option<(f64, f64)> {
    let shell = b.me_budget_shell_path.borrow().clone();
    let mut pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let mut dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let chapter = crate::media_probe::local_file_from_mpv(&b.mpv).or(shell);
    if let Some(ref ch) = chapter {
        let live = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
        if let Some((g, total)) = dvd_title_pos(b, ch, pos, live, dvd_bar) {
            pos = g;
            dur = total;
        }
    }
    pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let adj_u = seek.adjustment().upper();
    let adj_u = if adj_u.is_finite() { adj_u.max(0.0) } else { 0.0 };
    let len = if adj_u > 0.0 { adj_u } else if dur > 0.0 { dur } else { 0.0 };
    (len > 0.0).then_some((pos, len))
}

/// Steps **playback position** by `delta_sec` (e.g. −5 / +5 for arrow keys); keeps UI scale + clock aligned.
fn seek_arrow_step(d: &SeekArrowDeps<'_>, delta_sec: f64) {
    let nt = {
        let g = d.player.borrow();
        let Some(b) = g.as_ref() else {
            return;
        };
        let Some((pos, len)) = arrow_seek_pos_len(b, d.seek, d.dvd_bar) else {
            return;
        };
        (pos + delta_sec).clamp(0.0, len)
    };
    let s_abs = format!("{nt:.4}");
    main_player_seek_keyframes(
        &SeekKeyframeParams {
            player: d.player,
            gl: d.gl,
            smooth_seek_debounce: d.smooth_seek_debounce,
            resume_after_seek_idle: d.resume_after_seek_idle,
            play_toggle: d.play_toggle,
            dvd_bar: d.dvd_bar,
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
