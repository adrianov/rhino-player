/// Quiet period after `FileLoaded` / `VideoReconfig` / `path` / `container-fps` before
/// [smooth_60_full_resync_after_media_change]: mpv often emits those in separate drains; one timer
/// coalesces them so the bundled `.vpy` is not built twice with stale `container-fps` or SQLite ME rows.
/// **~160 ms** gives `estimated-vf-fps` more time to settle so NTSC film is less often misread as **24**
/// on the first attach (still not guaranteed on slow demux).
const SMOOTH_60_RESYNC_DEBOUNCE: Duration = Duration::from_millis(160);

/// Upserts **`media.decode_w/h`** when mpv already reports size — lets [crate::db::resolve_media_smooth_me_budget]
/// pick a same-resolution neighbor **before** the first Smooth rebuild (transport tick is too late).
fn sync_media_decode_row_for_me_budget(player: &Rc<RefCell<Option<MpvBundle>>>) {
    with_bundle(player, |b| {
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
    });
}

fn schedule_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    // Warm preload behind the continue grid: defer VapourSynth until reveal/unpause.
    if ctx.recent_visible.get() {
        return;
    }
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending())
    {
        return;
    }
    sync_media_decode_row_for_me_budget(&ctx.player);
    drop_glib_source(ctx.smooth_60_resync_debounce.as_ref());
    let deb = Rc::clone(&ctx.smooth_60_resync_debounce);
    let c = Rc::clone(ctx);
    *ctx.smooth_60_resync_debounce.borrow_mut() = Some(glib::timeout_add_local(
        SMOOTH_60_RESYNC_DEBOUNCE,
        move || {
            *deb.borrow_mut() = None;
            smooth_60_full_resync_after_media_change(&c.player, &c.eof.gl, &c.eof.reapply_60);
            glib::ControlFlow::Break
        },
    ));
}

/// Run `f` with the active mpv bundle if it is borrowable and present. Skips silently
/// otherwise (the player is `None` before GL realize, or already mutably borrowed by
/// another transport handler — both cases are normal in this dispatch path).
fn with_bundle(player: &Rc<RefCell<Option<MpvBundle>>>, f: impl FnOnce(&MpvBundle)) {
    if let Ok(g) = player.try_borrow() {
        if let Some(b) = g.as_ref() {
            f(b);
        }
    }
}

fn has_open_path(mpv: &Mpv) -> bool {
    matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty())
}

fn sync_smooth_vf_on_pause_transition(ctx: &Rc<TransportCtx>, paused: bool) {
    // Smooth 60: keep vapoursynth `vf` across pause/unpause so FlowFPS is not torn down and rebuilt
    // on every tap of Space. Seeks while paused still strip via [main_player_seek_keyframes] /
    // [video_pref::unload_smooth_on_pause].
    with_bundle(&ctx.player, |b| {
        if !has_open_path(&b.mpv) {
            return;
        }
        if !paused {
            // Same debounced path as `FileLoaded` / `container-fps` — avoids double `apply_mpv_video`
            // when unpause races a pending resync timer.
            schedule_smooth_60_resync_idle(ctx);
        }
    });
    ctx.eof.gl.queue_render();
}

/// Dispatch property-change / FileLoaded / VideoReconfig / PathChanged / `container-fps` events.
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
            sync_smooth_vf_on_pause_transition(ctx, p);
            ctx.blackout.sync();
        }
        TransportEv::Duration(d) => {
            let d = if d.is_finite() { d } else { 0.0 };
            if d > 0.0 {
                maybe_refresh_dvd_bar_cache(ctx);
                if !ctx.recent_visible.get() {
                    try_apply_pending_resume(ctx);
                }
            }
            let bar_d = dvd_bar_duration(ctx).unwrap_or(d);
            ctx.cache.borrow_mut().duration = bar_d;
            sync_seek_range(w, bar_d);
            sync_duration_label(w, bar_d);
            sync_speed_header(&ctx.player, w, d);
            refresh_play_button(ctx);
            sync_seek_chapters(ctx);
            if ctx.recent_visible.get() && d > 0.0 {
                schedule_warm_path_settle(Rc::clone(&ctx.player));
            }
        }
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::FileLoaded => dispatch_file_loaded(ctx),
        TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            sync_seek_chapters(ctx);
            // Often follows `FileLoaded` in a later drain — debounce merges into one vf rebuild.
            schedule_smooth_60_resync_idle(ctx);
        }
        TransportEv::PathChanged => {
            crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
            crate::video_pref::smooth_budget_reset_session_on_new_media(&ctx.smooth_budget_decoder);
            refresh_dvd_bar_cache(ctx);
            ctx.eof.sibling_seof.done.set(false);
            refresh_sibling_nav(ctx);
            sync_window_title_from_context(ctx);
            if !ctx.recent_visible.get() {
                try_apply_pending_resume(ctx);
            }
            transport_tick(ctx);
            schedule_smooth_60_resync_idle(ctx);
            sync_seek_chapters(ctx);
            if ctx.recent_visible.get() {
                schedule_warm_path_settle(Rc::clone(&ctx.player));
            }
        }
        TransportEv::ContainerFpsChanged => {
            schedule_smooth_60_resync_idle(ctx);
        }
    }
    mpris_enqueue_snapshot(ctx);
}

/// Recomputes Prev/Next sensitivity + tooltips. Called on `path`/`FileLoaded`/`VideoReconfig`.
fn refresh_sibling_nav(ctx: &Rc<TransportCtx>) {
    let cur = ctx.eof.last_path.borrow().clone();
    ctx.sibling_nav
        .refresh(cur.as_deref(), ctx.eof.sibling_seof.as_ref());
}

fn sync_window_aspect_from_player(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win_aspect: &Rc<Cell<Option<f64>>>,
) {
    with_bundle(player, |b| {
        sync_window_aspect_from_mpv(&b.mpv, win_aspect.as_ref());
    });
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
        &ctx.video_pref,
        &e.idle_inhib,
        &e.mpv_teardown_after_draw,
        &e.on_video_chrome,
        Rc::clone(&e.win_aspect),
        Some(Rc::clone(&e.on_file_loaded)),
        e.hdr_title_mirror.clone(),
        Rc::clone(&e.playback_focus),
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

include!("dispatch_sync_ui_file_loaded.rs");
include!("dispatch_sync_ui_speed.rs");
include!("dispatch_sync_ui_volume.rs");

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

fn sync_seek_range(w: &TransportWidgets, dur: f64) {
    let has_media = dur > 0.0;
    if w.seek.is_sensitive() != has_media {
        w.seek.set_sensitive(has_media);
    }
    if has_media && (w.seek_adj.upper() - dur).abs() > f64::EPSILON {
        w.seek_sync.set(true);
        w.seek_adj.set_lower(0.0);
        w.seek_adj.set_upper(dur);
        w.seek_sync.set(false);
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

fn set_tooltip_if_changed(w: &gtk::Widget, tip: &str) {
    if w.tooltip_text().as_deref() != Some(tip) {
        w.set_tooltip_text(Some(tip));
    }
}
