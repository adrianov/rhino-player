/// Coalesces Smooth 60 `vf` rebuild: `FileLoaded` and `path` updates often arrive in one drain;
/// prev/next, sibling EOF, and Open all reach mpv via `loadfile` → those events, plus **`container-fps`**
/// once the demuxer exposes cadence (often after the first Smooth idle).
/// **`VideoReconfig`** usually fires in a **later** drain after the decoder settles — scheduling again then
/// re-runs [smooth_60_full_resync_after_media_change] because `smooth_60_resync_idle_pending` was cleared
/// when the earlier idle ran (fixes stale cadence / A/V drift if we attached Smooth too early).
/// A second schedule **within the same burst** before that idle runs is still skipped (`pending` guard).
fn schedule_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    if ctx.smooth_60_resync_idle_pending.replace(true) {
        return;
    }
    let c = Rc::clone(ctx);
    let _ = glib::idle_add_local_once(move || {
        c.smooth_60_resync_idle_pending.set(false);
        smooth_60_full_resync_after_media_change(&c.player, &c.eof.gl, &c.eof.reapply_60);
    });
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

fn sync_seek_chapters(ctx: &Rc<TransportCtx>) {
    let mut list = Vec::new();
    if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            list = crate::chapter_list::mpv_chapter_list(&b.mpv);
        }
    }
    *ctx.seek_chapters.borrow_mut() = list;
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
            smooth_vf_attach_if_playing(
                Rc::clone(&ctx.player),
                ctx.eof.gl.clone(),
                ctx.eof.reapply_60.clone(),
                true,
            );
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
        }
        TransportEv::Duration(d) => {
            let d = if d.is_finite() { d } else { 0.0 };
            ctx.cache.borrow_mut().duration = d;
            sync_seek_range(w, d);
            sync_duration_label(w, d);
            sync_speed_header(&ctx.player, w, d);
            refresh_play_button(ctx);
            sync_seek_chapters(ctx);
        }
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::FileLoaded => {
            // Invalidate bundled ME budget fast-path (`vf_smooth_matches_prefs`) so **`apply_mpv_video`**
            // reinstalls vapoursynth: a warm VapourSynth interpreter reused across **`loadfile`** does not adopt
            // a newer **`RHINO_SMOOTH_MAX_AREA`** unless **`vf clr`/`vf add`** runs (**`smooth_vf_me_budget_applied`**).
            crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
            // New file: apply the SQLite-driven resume, restore the saved audio track *before*
            // any unpause so mpv does not play the default `aid` for a fraction of a second and
            // then switch (audio path re-open caused lip-sync drift on continue-grid → reopen).
            // Reset the one-shot EOF guard and let `transport_tick` pick up state.
            with_bundle(&ctx.player, |b| {
                b.apply_pending_resume();
                audio_tracks::restore_saved_audio(&b.mpv);
                audio_tracks::ensure_playable_audio(&b.mpv);
            });
            ctx.eof.sibling_seof.done.set(false);
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            schedule_transport_resync_on_idle(ctx);
            schedule_smooth_60_resync_idle(ctx);
            sync_seek_chapters(ctx);
        }
        TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            sync_seek_chapters(ctx);
            // Usually follows `FileLoaded` in a later drain — second idle reapplies Smooth once output
            // stabilizes (cadence / vf timing); skipped when redundant with an unserviced pending idle.
            schedule_smooth_60_resync_idle(ctx);
        }
        TransportEv::PathChanged => {
            crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
            ctx.eof.sibling_seof.done.set(false);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            schedule_smooth_60_resync_idle(ctx);
            sync_seek_chapters(ctx);
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

fn set_tooltip_if_changed(w: &gtk::Widget, tip: &str) {
    if w.tooltip_text().as_deref() != Some(tip) {
        w.set_tooltip_text(Some(tip));
    }
}
