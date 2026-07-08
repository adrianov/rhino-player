enum ReattachNeed {
    Yes,
    No,
    /// Pause(false) arrived while [TransportCtx::player] was already borrowed (load / drain).
    BorrowBusy,
}

/// Unpause needs a Smooth resync only when the graph is missing / was stripped (Smooth on),
/// or a stale graph must be removed (Smooth off). Plain pause→resume with a live graph skips it.
fn smooth_needs_reattach_on_unpause(ctx: &Rc<TransportCtx>) -> ReattachNeed {
    // try_borrow: pause events may be dispatched while the bundle is already borrowed.
    let Ok(g) = ctx.player.try_borrow() else {
        return ReattachNeed::BorrowBusy;
    };
    let Some(b) = g.as_ref() else {
        return ReattachNeed::No;
    };
    if !has_open_path(&b.mpv) {
        return ReattachNeed::No;
    }
    let has_vf = crate::video_pref::vf_chain_has_vapoursynth(&b.mpv);
    if !ctx.video_pref.borrow().smooth_60 {
        return if has_vf {
            ReattachNeed::Yes
        } else {
            ReattachNeed::No
        };
    }
    if b.smooth_vf_stripped_this_open() || !has_vf {
        ReattachNeed::Yes
    } else {
        ReattachNeed::No
    }
}

fn sync_smooth_vf_on_pause_transition(ctx: &Rc<TransportCtx>, paused: bool) {
    if !paused {
        match smooth_needs_reattach_on_unpause(ctx) {
            ReattachNeed::Yes => schedule_smooth_60_resync_idle(ctx),
            ReattachNeed::BorrowBusy if ctx.video_pref.borrow().smooth_60 => {
                let c = Rc::clone(ctx);
                glib::idle_add_local_once(move || {
                    if matches!(smooth_needs_reattach_on_unpause(&c), ReattachNeed::Yes) {
                        schedule_smooth_60_resync_idle(&c);
                    }
                });
            }
            _ => {}
        }
    }
    ctx.eof.gl.queue_render();
}

fn dispatch_duration_event(ctx: &Rc<TransportCtx>, raw: f64) {
    let w = &ctx.widgets;
    let mut d = if raw.is_finite() { raw } else { 0.0 };
    if d > 0.0 {
        maybe_refresh_dvd_bar_cache(ctx);
        if !ctx.recent_visible.get() {
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
    }
    if let Some(ch) = transport_chapter_path_for_ctx(ctx) {
        if crate::playback_entity::PlaybackEntity::resolve(&ch).has_unified_timeline() {
            d = crate::dvd_vob_timeline::clamp_vob_duration(d);
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
        TransportEv::Duration(d) => dispatch_duration_event(ctx, d),
        TransportEv::Volume(v) => sync_volume(w, v),
        TransportEv::Mute(m) => sync_mute(w, m),
        TransportEv::VolumeMax(vmax) => sync_volume_max(w, vmax),
        TransportEv::FileLoaded => dispatch_file_loaded(ctx),
        TransportEv::VideoReconfig => {
            sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
            refresh_sibling_nav(ctx);
            transport_tick(ctx);
            sync_seek_chapters(ctx);
            crate::video_fill::request_fill_resync();
            schedule_smooth_60_resync_idle(ctx);
        }
        TransportEv::PathChanged => {
            crate::video_fill::request_fill_reset();
            crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
            crate::video_pref::smooth_budget_reset_session_on_new_media(&ctx.smooth_budget_decoder);
            refresh_dvd_bar_cache(ctx);
            ctx.eof.sibling_seof.done.set(false);
            ctx.eof.sibling_seof.reset_playback_span();
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
            sync_audio_tooltip(ctx);
        }
        TransportEv::ContainerFpsChanged => schedule_smooth_60_resync_idle(ctx),
    }
    mpris_enqueue_snapshot(ctx);
}

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
    );
}

fn refresh_play_button(ctx: &Rc<TransportCtx>) {
    let (dur, paused) = {
        let c = ctx.cache.borrow();
        (c.duration, c.pause)
    };
    sync_play_button(&ctx.widgets, dur, paused);
}
