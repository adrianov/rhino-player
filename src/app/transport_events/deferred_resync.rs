/// 1 Hz transport tick: read live mpv state (time-pos, duration, pause, core-idle), update UI,
/// and trigger sibling advance when `core-idle && near end of file`.
///
/// Replaces several event-driven paths that mpv does not deliver reliably at high speed:
/// `time-pos` updates, `core-idle` change, `eof-reached` flip, and `EndFile(Eof)` (none of those
/// fire dependably with `keep-open=yes` at 8×). Coarser 1 s resolution is enough for the seek bar
/// and clock; sibling advance fires within a second of mpv stalling at the tail.
use gtk::gdk::prelude::ToplevelExt;
use gtk::prelude::NativeExt;

include!("dvd_transport_periodic_log.rs");

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
    #[cfg(target_os = "macos")]
    if crate::macos_header_menu::any_open() {
        return;
    }
    let Ok(g) = player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        if !label.text().is_empty() {
            label.set_text("");
        }
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    crate::sub_tracks::refresh_sub_header(&b.mpv, label, shell.as_deref());
}

fn install_transport_tick(ctx: &Rc<TransportCtx>) {
    drop_glib_source(ctx.tick.as_ref());
    let c = Rc::clone(ctx);
    *ctx.tick.borrow_mut() = Some(glib::timeout_add_local(TICK_INTERVAL, move || {
        transport_tick(&c);
        glib::ControlFlow::Continue
    }));
}

/// Keep mpv paused while the continue strip is up and playback has not been revealed.
fn hold_browse_pause(ctx: &TransportCtx, browse: bool) {
    if !browse || ctx.eof.playback_focus.get() {
        return;
    }
    hold_browse_pause_player(&ctx.player, browse);
}

fn hold_browse_pause_player(player: &Rc<RefCell<Option<MpvBundle>>>, browse: bool) {
    if !browse {
        return;
    }
    let Ok(g) = player.try_borrow() else {
        return;
    };
    if let Some(b) = g.as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }
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

fn chapter_cross_load_busy(ctx: &TransportCtx) -> bool {
    ctx.player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_cross_load_busy())
}

fn transport_tick(ctx: &Rc<TransportCtx>) {
    #[cfg(target_os = "macos")]
    sync_macos_now_playing_for_transport(&ctx.player);

    maybe_refresh_dvd_bar_cache(ctx);

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
    let bar_visible = ctx.bar_show.get() || ctx.recent_visible.get();
    update_time_labels(&ctx.widgets, pos, dur);
    sync_duration_label(&ctx.widgets, dur);
    sync_seek_range(&ctx.widgets, dur);
    sync_speed_header(&ctx.player, &ctx.widgets, dur);
    refresh_play_button(ctx);
    if bar_visible {
        sync_seek_pos(&ctx.widgets, pos, dur);
    }
    sync_decode_size_on_tick(&ctx.player);
    let browse = crate::app::browse_overlay_active(&ctx.eof.recent);
    hold_browse_pause(ctx, browse);
    // Mid-title DVD chapter EOF: detect local tail every tick — mpv often keeps `core-idle=false`
    // and `eof-reached=false` with `keep-open=yes` (and Smooth `vf`) at a `.vob` boundary.
    if !browse && maybe_advance_dvd_chapter_eof(ctx) {
        // advanced; skip title-end sibling advance this tick
    } else if natural_eof_for_advance(ctx, core_idle)
        && !browse
        && !chapter_cross_load_busy(ctx)
        && sibling_eof_ready(ctx, dur, pos, core_idle)
    {
        run_sibling_eof(ctx);
    }
    sync_sub_header_readout(&ctx.player, &ctx.widgets.sub_readout);
    stamp_smooth_toolbar_readout(
        Some(&ctx.widgets.smooth_toolbar_status),
        Some(&ctx.widgets.smooth_toolbar_btn),
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
    maybe_dvd_transport_periodic_log(ctx, pos, dur);
}

include!("transport_read_state.rs");

fn natural_eof_for_advance(ctx: &TransportCtx, core_idle: bool) -> bool {
    if core_idle {
        return true;
    }
    let Ok(g) = ctx.player.try_borrow() else {
        return false;
    };
    let Some(b) = g.as_ref() else {
        return false;
    };
    b.mpv.get_property::<bool>("eof-reached").unwrap_or(false)
}

fn sibling_eof_ready(ctx: &TransportCtx, dur: f64, pos: f64, core_idle: bool) -> bool {
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending())
    {
        return false;
    }
    let eof_reached = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.mpv.get_property::<bool>("eof-reached").unwrap_or(false));
    if !ctx.eof.sibling_seof.played_into_tail(dur, eof_reached) {
        return false;
    }
    dvd_eof_tail(ctx, dur, pos, core_idle)
}

fn maybe_advance_dvd_chapter_eof(ctx: &Rc<TransportCtx>) -> bool {
    if crate::app::browse_overlay_active(&ctx.eof.recent) {
        return false;
    }
    {
        let Ok(g) = ctx.player.try_borrow() else {
            return false;
        };
        let Some(b) = g.as_ref() else {
            return false;
        };
        let shell = b.me_budget_shell_path.borrow();
        crate::dvd_vob_timeline::refresh_dvd_bar_at_chapter_eof(
            &ctx.dvd_bar,
            &b.mpv,
            shell.as_deref(),
        );
    }
    let advanced = {
        let bar = ctx.dvd_bar.borrow();
        let Some(ref bar) = *bar else {
            return false;
        };
        crate::dvd_vob_timeline::advance_title_chapter_eof(&ctx.player, bar)
    };
    if !advanced {
        return false;
    }
    crate::app::transport_drain_after_loadfile_idle();
    true
}

fn dvd_eof_tail(ctx: &TransportCtx, bar_dur: f64, bar_pos: f64, core_idle: bool) -> bool {
    let Ok(g) = ctx.player.try_borrow() else {
        return false;
    };
    let Some(b) = g.as_ref() else {
        return false;
    };
    let bar = ctx.dvd_bar.borrow();
    crate::dvd_vob_timeline::title_eof_for_sibling_advance(
        &b.mpv,
        bar.as_ref(),
        bar_dur,
        bar_pos,
        core_idle,
    )
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
