/// 1 Hz transport tick: read live mpv state (time-pos, duration, pause, core-idle), update UI,
/// and trigger sibling advance when `core-idle && near end of file`.
///
/// Replaces several event-driven paths that mpv does not deliver reliably at high speed:
/// `time-pos` updates, `core-idle` change, `eof-reached` flip, and `EndFile(Eof)` (none of those
/// fire dependably with `keep-open=yes` at 8×). Coarser 1 s resolution is enough for the seek bar
/// and clock; sibling advance fires within a second of mpv stalling at the tail.
fn install_transport_tick(ctx: &Rc<TransportCtx>) {
    if let Some(id) = ctx.tick.borrow_mut().take() {
        id.remove();
    }
    let c = Rc::clone(ctx);
    *ctx.tick.borrow_mut() = Some(glib::timeout_add_local(TICK_INTERVAL, move || {
        transport_tick(&c);
        glib::ControlFlow::Continue
    }));
}

fn transport_tick(ctx: &Rc<TransportCtx>) {
    let Some((pause, core_idle, dur, pos)) = read_transport_state(&ctx.player) else {
        return;
    };
    {
        let mut c = ctx.cache.borrow_mut();
        c.pause = pause;
        c.core_idle = core_idle;
        c.duration = dur;
        c.pos = pos;
    }
    let bar_visible = ctx.bar_show.get() || ctx.recent_visible.get();
    update_time_labels(&ctx.widgets, pos, dur);
    sync_seek_range(&ctx.widgets, dur);
    sync_speed_button(&ctx.widgets, dur);
    refresh_play_button(ctx);
    if bar_visible {
        sync_seek_pos(&ctx.widgets, pos, dur);
    }
    if core_idle && dur > 0.0 && (dur - pos) <= TICK_EOF_TAIL_SEC {
        run_sibling_eof(ctx);
    }
}

fn read_transport_state(
    player: &Rc<RefCell<Option<MpvBundle>>>,
) -> Option<(bool, bool, f64, f64)> {
    let g = player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let core_idle = b.mpv.get_property::<bool>("core-idle").unwrap_or(false);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    Some((pause, core_idle, dur, pos))
}

fn schedule_transport_resync_on_idle(ctx: &Rc<TransportCtx>) {
    if ctx.idle_resync_pending.replace(true) {
        return;
    }
    let c = Rc::clone(ctx);
    let _ = glib::idle_add_local_once(move || {
        c.idle_resync_pending.set(false);
        transport_tick(&c);
    });
}
