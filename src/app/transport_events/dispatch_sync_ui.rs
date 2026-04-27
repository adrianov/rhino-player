fn dispatch_event(ctx: &Rc<TransportCtx>, ev: TransportEv) {
    let w = &ctx.widgets;
    if std::env::var_os("RHINO_TRANSPORT_TRACE").is_some() {
        eprintln!("[rhino] transport ev: {ev:?}");
    }
    match ev {
        TransportEv::Pause(p) => {
            ctx.cache.borrow_mut().pause = p;
            sync_play_button(w, ctx.cache.borrow().duration, p);
        }
        TransportEv::Duration(d) => {
            let d = if d.is_finite() { d } else { 0.0 };
            ctx.cache.borrow_mut().duration = d;
            sync_seek_range(w, d);
            sync_play_button(w, d, ctx.cache.borrow().pause);
            sync_speed_button(w, d);
        }
        TransportEv::TimePos(p) => apply_time_pos(ctx, p),
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::EofReached(eof) => {
            ctx.cache.borrow_mut().eof = eof;
            if eof {
                run_sibling_eof(ctx);
            }
        }
        TransportEv::EndFile => run_sibling_eof(ctx),
        TransportEv::FileLoaded | TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            resync_play_button(ctx);
        }
        TransportEv::PathChanged => {
            refresh_sibling_nav(ctx);
            resync_play_button(ctx);
        }
    }
}

/// On `FileLoaded` / `VideoReconfig`, mpv may have already emitted the new `pause` /
/// `duration` values before the observer was installed (warm preload), or the events
/// may have been coalesced. Re-read both properties straight from mpv so the play
/// button always reflects the actual playback state without waiting for the next event.
fn resync_play_button(ctx: &Rc<TransportCtx>) {
    let g = match ctx.player.try_borrow() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let dur = b.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let dur = if dur.is_finite() { dur } else { 0.0 };
    {
        let mut c = ctx.cache.borrow_mut();
        c.pause = pause;
        c.duration = dur;
    }
    sync_play_button(&ctx.widgets, dur, pause);
    sync_speed_button(&ctx.widgets, dur);
    sync_seek_range(&ctx.widgets, dur);
}

/// Recomputes Prev/Next sensitivity + tooltips. Called on `path`/`FileLoaded`/`VideoReconfig`
/// instead of the previous 200ms poll, so the bottom-bar nav always reflects the loaded file.
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

fn apply_time_pos(ctx: &Rc<TransportCtx>, p: f64) {
    let dur = {
        let mut c = ctx.cache.borrow_mut();
        c.pos = p;
        c.duration
    };
    let bar_visible = ctx.bar_show.get() || ctx.recent_visible.get();
    if bar_visible {
        update_time_labels(&ctx.widgets, p, dur);
    }
    let now = Instant::now();
    let allow = {
        let c = ctx.cache.borrow();
        c.last_pos_apply
            .map(|t| now.duration_since(t) >= TIME_POS_MIN_GAP)
            .unwrap_or(true)
    };
    if !allow || !bar_visible {
        return;
    }
    ctx.cache.borrow_mut().last_pos_apply = Some(now);
    sync_seek_pos(&ctx.widgets, p, dur);
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

fn update_tail_timer(ctx: &Rc<TransportCtx>) {
    let cache = ctx.cache.borrow();
    let needs = !cache.pause
        && cache.duration > 0.0
        && cache.pos.is_finite()
        && (cache.duration - cache.pos) <= SIBLING_END_SLACK_SEC;
    drop(cache);
    let mut slot = ctx.tail_timer.borrow_mut();
    if needs {
        if slot.is_some() {
            return;
        }
        let ctx_t = ctx.clone();
        let id = glib::timeout_add_local(TAIL_STALL_INTERVAL, move || {
            run_sibling_eof(&ctx_t);
            let c = ctx_t.cache.borrow();
            let still = !c.pause
                && c.duration > 0.0
                && c.pos.is_finite()
                && (c.duration - c.pos) <= SIBLING_END_SLACK_SEC;
            if still {
                glib::ControlFlow::Continue
            } else {
                *ctx_t.tail_timer.borrow_mut() = None;
                glib::ControlFlow::Break
            }
        });
        *slot = Some(id);
    } else if let Some(id) = slot.take() {
        id.remove();
    }
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
    if dur <= 0.0 || !pos.is_finite() {
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

fn update_time_labels(w: &TransportWidgets, pos: f64, dur: f64) {
    let pos_s = format_time(pos);
    if w.time_left.label().as_str() != pos_s {
        w.time_left.set_label(&pos_s);
    }
    let dur_s = format_time(dur);
    if w.time_right.label().as_str() != dur_s {
        w.time_right.set_label(&dur_s);
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
