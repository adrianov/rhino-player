/// 1 Hz transport tick: read live mpv state (time-pos, duration, pause, core-idle), update UI,
/// and trigger sibling advance when `core-idle && near end of file`.
///
/// Replaces several event-driven paths that mpv does not deliver reliably at high speed:
/// `time-pos` updates, `core-idle` change, `eof-reached` flip, and `EndFile(Eof)` (none of those
/// fire dependably with `keep-open=yes` at 8×). Coarser 1 s resolution is enough for the seek bar
/// and clock; sibling advance fires within a second of mpv stalling at the tail.
use gtk::gdk::prelude::ToplevelExt;
use gtk::prelude::NativeExt;

/// **`smooth_budget`** uses presentation strain; GTK / compositors mis-read it when unfocused,
/// minimized, or unmapped. Skip **both** ladders until the playback shell suits pacing again.
#[must_use]
fn smooth_budget_transport_window_ticks_count(win: &adw::ApplicationWindow) -> bool {
    if !win.is_visible() || !win.is_mapped() || !win.is_active() {
        return false;
    }
    !win.surface().is_some_and(|s| {
        s.downcast_ref::<gtk::gdk::Toplevel>()
            .is_some_and(|t| t.state().contains(gtk::gdk::ToplevelState::MINIMIZED))
    })
}

fn sync_sub_header_readout(player: &Rc<RefCell<Option<MpvBundle>>>, label: &gtk::Label) {
    let Ok(g) = player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        if !label.text().is_empty() {
            label.set_text("");
        }
        return;
    };
    crate::sub_tracks::refresh_sub_header(&b.mpv, label);
}

fn install_transport_tick(ctx: &Rc<TransportCtx>) {
    drop_glib_source(ctx.tick.as_ref());
    let c = Rc::clone(ctx);
    *ctx.tick.borrow_mut() = Some(glib::timeout_add_local(TICK_INTERVAL, move || {
        transport_tick(&c);
        glib::ControlFlow::Continue
    }));
}

fn sync_decode_size_on_tick(player: &Rc<RefCell<Option<MpvBundle>>>) {
    let Ok(g) = player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    if !b.may_persist_media_rows() {
        return;
    }
    let Some(p) = crate::media_probe::local_file_from_mpv(&b.mpv) else {
        return;
    };
    let Some((w, h)) = crate::video_pref::decode_wh_from_mpv(&b.mpv) else {
        return;
    };
    crate::db::media_sync_decode_size(&p, w, h);
}

fn transport_tick(ctx: &Rc<TransportCtx>) {
    #[cfg(target_os = "macos")]
    sync_macos_now_playing_for_transport(&ctx.player);

    let Some((pause, core_idle, dur, pos)) = read_transport_state(ctx) else {
        return;
    };
    {
        let mut c = ctx.cache.borrow_mut();
        c.pause = pause;
        c.core_idle = core_idle;
        c.duration = dur;
        c.pos = pos;
    }
    if !warm_transport_chrome_pending(ctx, dur) {
        let bar_visible = ctx.bar_show.get() || ctx.recent_visible.get();
        update_time_labels(&ctx.widgets, pos, dur);
        sync_duration_label(&ctx.widgets, dur);
        sync_seek_range(&ctx.widgets, dur);
        sync_speed_header(&ctx.player, &ctx.widgets, dur);
        refresh_play_button(ctx);
        if bar_visible {
            sync_seek_pos(&ctx.widgets, pos, dur);
        }
    }
    sync_decode_size_on_tick(&ctx.player);
    if core_idle && dur > 0.0 && (dur - pos) <= TICK_EOF_TAIL_SEC {
        run_sibling_eof(ctx);
    }
    sync_sub_header_readout(&ctx.player, &ctx.widgets.sub_readout);
    stamp_smooth_toolbar_readout(
        Some(&ctx.widgets.smooth_toolbar_status),
        &ctx.player,
    );
    if smooth_budget_transport_window_ticks_count(&ctx.eof.win) {
        crate::video_pref::smooth_budget_on_transport_tick(
            &ctx.player,
            &ctx.video_pref,
            pause,
            core_idle,
            ctx.smooth_budget_decoder.as_ref(),
        );
    }
    mpris_enqueue_snapshot(ctx);
    ctx.blackout.sync();
}

fn read_transport_state(ctx: &TransportCtx) -> Option<(bool, bool, f64, f64)> {
    let g = ctx.player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let core_idle = b.mpv.get_property::<bool>("core-idle").unwrap_or(false);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let pos = if pause && ctx.recent_visible.get() {
        b.knob_pos_from_sqlite()
    } else {
        let p = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
        if p.is_finite() { p.max(0.0) } else { 0.0 }
    };
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
