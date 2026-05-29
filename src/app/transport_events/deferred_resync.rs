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
        && dvd_eof_tail(ctx, dur, pos)
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

fn browse_pause_snap(
    ctx: &TransportCtx,
    shell: &Option<std::path::PathBuf>,
    pause: bool,
    pos: f64,
    dur: f64,
) -> (f64, f64, bool) {
    if !ctx.recent_visible.get() {
        return (pos, dur, false);
    }
    let Some(p) = shell.as_ref() else {
        return (pos, dur, false);
    };
    let Some(snap) =
        crate::media_probe::continue_grid_cache_lookup(&ctx.continue_grid_cache, p)
    else {
        return (pos, dur, false);
    };
    let mut pos = pos;
    let mut dur = dur;
    let mut from_entity = false;
    if pause {
        pos = snap.resume_sec;
        from_entity = true;
    }
    if dur <= 0.0 {
        dur = snap.duration_sec;
    }
    (pos, dur, from_entity)
}

fn read_transport_state(ctx: &TransportCtx) -> Option<(bool, bool, f64, f64)> {
    let mut g = ctx.player.try_borrow_mut().ok()?;
    let b = g.as_mut()?;
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let core_idle = b.mpv.get_property::<bool>("core-idle").unwrap_or(false);
    let mut dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let mut pos = b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    let shell = b.me_budget_shell_path.borrow().clone();
    let browse_chapter = crate::playback_entity::transport_chapter_path(
        ctx.recent_visible.get(),
        ctx.eof.last_path.borrow().clone(),
        Some(&b.mpv),
        shell.as_deref(),
    );
    let playback_chapter = crate::playback_entity::transport_chapter_path(
        false,
        None,
        Some(&b.mpv),
        shell.as_deref(),
    );
    let (mut pos, mut dur, pos_from_entity_snap) =
        browse_pause_snap(ctx, &browse_chapter, pause, pos, dur);
    let bar_chapter = playback_chapter.as_ref().or(browse_chapter.as_ref());
    if let Some(ch) = bar_chapter {
        let entity = crate::playback_entity::PlaybackEntity::resolve(ch);
        if entity.has_unified_timeline() {
            dur = crate::dvd_vob_timeline::clamp_vob_duration(dur);
        }
        let bar = ctx.dvd_bar.borrow();
        if pos_from_entity_snap {
            if let (Some(pb), Some(bar)) = (playback_chapter.as_ref(), bar.as_ref()) {
                if entity.dvd_bar_active(pb, bar) {
                    dur = bar.total_sec();
                }
            }
        } else if let Some(pb) = playback_chapter.as_ref().or(browse_chapter.as_ref()) {
            (dur, pos) = entity.transport_bar(pb, pos, dur, bar.as_ref(), Some(b));
        }
        let browse_overlay = ctx.recent_visible.get() && !ctx.eof.playback_focus.get();
        if entity.has_unified_timeline()
            && dur > 0.0
            && !pos_from_entity_snap
            && !browse_overlay
            && !b.resume_seek_pending()
        {
            b.set_transport_bar_persist(dur, pos);
        }
    }
    Some((pause, core_idle, dur, pos))
}

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

fn dvd_eof_tail(ctx: &TransportCtx, bar_dur: f64, bar_pos: f64) -> bool {
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
