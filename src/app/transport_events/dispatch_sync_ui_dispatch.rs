fn dispatch_duration_event(ctx: &Rc<TransportCtx>, raw: f64) {
    let w = &ctx.widgets;
    let d = if raw.is_finite() { raw } else { 0.0 };
    if d > 0.0 {
        apply_duration_resume_side_effects(ctx);
    }
    let d = clamp_duration_for_chapter_timeline(ctx, d);
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

fn apply_duration_resume_side_effects(ctx: &Rc<TransportCtx>) {
    maybe_refresh_dvd_bar_cache(ctx);
    if !ctx.recent_visible.get() {
        apply_resume_seek_and_resync_if_cleared(ctx);
    }
}

fn apply_resume_seek_and_resync_if_cleared(ctx: &Rc<TransportCtx>) {
    let resume_was_pending = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending());
    try_apply_pending_resume(ctx);
    let resume_cleared = resume_was_pending
        && !ctx
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.resume_seek_pending());
    if resume_cleared && ctx.video_pref.borrow().smooth_60 {
        schedule_smooth_60_resync_idle(ctx);
    }
}

fn clamp_duration_for_chapter_timeline(ctx: &TransportCtx, d: f64) -> f64 {
    if let Some(ch) = transport_chapter_path_for_ctx(ctx) {
        if crate::playback_entity::PlaybackEntity::resolve(&ch).has_unified_timeline() {
            return crate::dvd_vob_timeline::clamp_vob_duration(d);
        }
    }
    d
}

fn dispatch_event(ctx: &Rc<TransportCtx>, ev: TransportEv) {
    if std::env::var_os("RHINO_TRANSPORT_TRACE").is_some() {
        eprintln!("[rhino] transport ev: {ev:?}");
    }
    match ev {
        TransportEv::Duration(d) => dispatch_duration_event(ctx, d),
        TransportEv::FileLoaded => dispatch_file_loaded(ctx),
        TransportEv::LoadFailed => dispatch_load_failed(ctx),
        other => dispatch_simple_transport_ev(ctx, other),
    }
    mpris_enqueue_snapshot(ctx);
}

/// Arms that need only `ctx` (plus widgets) — keeps [dispatch_event] a pure router.
fn dispatch_simple_transport_ev(ctx: &Rc<TransportCtx>, ev: TransportEv) {
    let w = &ctx.widgets;
    match ev {
        TransportEv::Pause(p) => on_pause_event(ctx, p),
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::VideoReconfig => on_video_reconfig(ctx),
        TransportEv::PathChanged => on_path_changed(ctx),
        TransportEv::ContainerFpsChanged => schedule_smooth_60_resync_idle(ctx),
        _ => {}
    }
}

fn on_pause_event(ctx: &Rc<TransportCtx>, p: bool) {
    ctx.cache.borrow_mut().pause = p;
    refresh_play_button(ctx);
    sync_smooth_vf_on_pause_transition(ctx, p);
    ctx.blackout.sync();
}

include!("dispatch_sync_ui_pause_smooth.rs");
include!("dispatch_sync_ui_media_change.rs");

fn refresh_sibling_nav(ctx: &Rc<TransportCtx>) {
    let cur = ctx.eof.last_path.borrow().clone();
    ctx.sibling_nav
        .refresh(cur.as_deref(), ctx.eof.sibling_seof.as_ref());
}

fn sync_window_aspect_from_player(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win_aspect: &Rc<WinAspectCell>,
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
        &e.on_open_fail,
    );
}

fn refresh_play_button(ctx: &Rc<TransportCtx>) {
    let (dur, paused) = {
        let c = ctx.cache.borrow();
        (c.duration, c.pause)
    };
    sync_play_button(&ctx.widgets, dur, paused);
}
