/// Dispatch property-change / FileLoaded / VideoReconfig / PathChanged events.
/// Time-pos, core-idle, and EOF detection live in [transport_tick] (see deferred_resync.rs).
fn dispatch_event(ctx: &Rc<TransportCtx>, ev: TransportEv) {
    let w = &ctx.widgets;
    if std::env::var_os("RHINO_TRANSPORT_TRACE").is_some() {
        eprintln!("[rhino] transport ev: {ev:?}");
    }
    match ev {
        TransportEv::Pause(p) => {
            ctx.cache.borrow_mut().pause = p;
            refresh_play_button(ctx);
        }
        TransportEv::Duration(d) => {
            let d = if d.is_finite() { d } else { 0.0 };
            ctx.cache.borrow_mut().duration = d;
            sync_seek_range(w, d);
            sync_duration_label(w, d);
            sync_speed_button(w, d);
            refresh_play_button(ctx);
        }
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::FileLoaded => {
            // New file: apply the SQLite-driven resume (if any), reset the one-shot EOF guard,
            // and let `transport_tick` pick up state.
            if let Ok(g) = ctx.player.try_borrow() {
                if let Some(b) = g.as_ref() {
                    b.apply_pending_resume();
                }
            }
            ctx.eof.sibling_seof.done.set(false);
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            schedule_transport_resync_on_idle(ctx);
        }
        TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
        }
        TransportEv::PathChanged => {
            ctx.eof.sibling_seof.done.set(false);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
        }
    }
}

/// Recomputes Prev/Next sensitivity + tooltips. Called on `path`/`FileLoaded`/`VideoReconfig`.
fn refresh_sibling_nav(ctx: &Rc<TransportCtx>) {
    let cur = current_local_path(&ctx.player).or_else(|| ctx.eof.last_path.borrow().clone());
    ctx.sibling_nav
        .refresh(cur.as_deref(), ctx.eof.sibling_seof.as_ref());
}

fn current_local_path(player: &Rc<RefCell<Option<MpvBundle>>>) -> Option<PathBuf> {
    let g = player.try_borrow().ok()?;
    let b = g.as_ref()?;
    local_file_from_mpv(&b.mpv)
}

fn sync_window_aspect_from_player(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win_aspect: &Rc<Cell<Option<f64>>>,
) {
    let g = match player.try_borrow() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(b) = g.as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, win_aspect.as_ref());
    }
}

fn run_sibling_eof(ctx: &Rc<TransportCtx>) {
    let e = &ctx.eof;
    maybe_advance_sibling_on_eof(
        &ctx.player,
        &e.win,
        &e.gl,
        &e.recent,
        &e.last_path,
        e.sibling_seof.as_ref(),
        &e.exit_after_current,
        &e.app,
        &e.sub_pref,
        &e.idle_inhib,
        &e.on_video_chrome,
        Rc::clone(&e.win_aspect),
        Some(Rc::clone(&e.on_file_loaded)),
        &e.reapply_60,
    );
}

/// Refresh play/pause icon from `cache.pause` — the user's explicit intent. The optimistic
/// click handler in [crate::app::recent_undo::flip_play_icon] writes the same icon, so the
/// tick reconciliation never flickers the button after a toggle. Brief decoder priming
/// (`core-idle=true` right after un-pause) is no longer factored in, and EOF stalls are
/// resolved by sibling advance within `TICK_EOF_TAIL_SEC`.
fn refresh_play_button(ctx: &Rc<TransportCtx>) {
    let (dur, paused) = {
        let c = ctx.cache.borrow();
        (c.duration, c.pause)
    };
    sync_play_button(&ctx.widgets, dur, paused);
}

fn sync_play_button(w: &TransportWidgets, dur: f64, paused: bool) {
    let has_media = dur > 0.0;
    if w.play_pause.is_sensitive() != has_media {
        w.play_pause.set_sensitive(has_media);
    }
    let (icon, tip) = if has_media && !paused {
        ("media-playback-pause-symbolic", "Pause (Space)")
    } else if has_media {
        ("media-playback-start-symbolic", "Play (Space)")
    } else {
        ("media-playback-start-symbolic", "No media")
    };
    if w.play_pause.icon_name().as_deref() != Some(icon) {
        w.play_pause.set_icon_name(icon);
    }
    set_tooltip_if_changed(w.play_pause.upcast_ref::<gtk::Widget>(), tip);
}

fn sync_speed_button(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.speed_menu.is_sensitive() != has_media {
        w.speed_menu.set_sensitive(has_media);
    }
}

fn sync_seek_range(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.seek.is_sensitive() != has_media {
        w.seek.set_sensitive(has_media);
    }
    if has_media && (w.seek_adj.upper() - dur).abs() > f64::EPSILON {
        w.seek_adj.set_lower(0.0);
        w.seek_adj.set_upper(dur);
    }
}

fn sync_seek_pos(w: &TransportWidgets, pos: f64, dur: f64) {
    if dur <= 0.0 || !pos.is_finite() || w.seek_grabbed.get() {
        return;
    }
    let v = pos.clamp(0.0, dur);
    if (w.seek_adj.value() - v).abs() < 0.01 {
        return;
    }
    w.seek_sync.set(true);
    w.seek_adj.set_value(v);
    w.seek_sync.set(false);
}

/// Updates the bottom-left clock from mpv's `time-pos`. The right-hand duration label is
/// updated by [sync_duration_label] from the `Duration` property event and on each
/// [transport_tick] once mpv reports a length (covers demuxers that expose duration after load).
fn update_time_labels(w: &TransportWidgets, pos: f64, _dur: f64) {
    if w.seek_grabbed.get() {
        return;
    }
    let pos_s = format_time(pos);
    if w.time_left.text().as_str() != pos_s {
        w.time_left.set_text(&pos_s);
    }
}

fn sync_duration_label(w: &TransportWidgets, dur: f64) {
    let dur_s = format_time(dur);
    if w.time_right.text().as_str() != dur_s {
        w.time_right.set_text(&dur_s);
    }
}

fn sync_volume(w: &TransportWidgets, vol: f64) {
    let muted = w.vol_mute.is_active();
    let v_icon = vol_icon(muted, vol);
    if w.vol_menu.icon_name().as_deref() != Some(v_icon) {
        w.vol_menu.set_icon_name(v_icon);
    }
    if w.vol_menu.is_active() {
        return;
    }
    let clamped = vol.clamp(0.0, w.vol_adj.upper());
    if (w.vol_adj.value() - clamped).abs() < 0.01 {
        return;
    }
    w.vol_sync.set(true);
    w.vol_adj.set_value(clamped);
    w.vol_sync.set(false);
}

fn sync_mute(w: &TransportWidgets, muted: bool) {
    let icon = vol_mute_pop_icon(muted);
    if w.vol_mute.icon_name().as_deref() != Some(icon) {
        w.vol_mute.set_icon_name(icon);
    }
    if w.vol_mute.is_active() != muted {
        w.vol_sync.set(true);
        w.vol_mute.set_active(muted);
        w.vol_sync.set(false);
    }
    set_tooltip_if_changed(
        w.vol_mute.upcast_ref::<gtk::Widget>(),
        if muted { "Unmute" } else { "Mute" },
    );
}

fn sync_volume_max(w: &TransportWidgets, vmax: f64) {
    if vmax.is_finite() && vmax > 0.0 && (w.vol_adj.upper() - vmax).abs() > f64::EPSILON {
        w.vol_adj.set_upper(vmax);
    }
}

fn set_tooltip_if_changed(w: &gtk::Widget, tip: &str) {
    if w.tooltip_text().as_deref() != Some(tip) {
        w.set_tooltip_text(Some(tip));
    }
}
