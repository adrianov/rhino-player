fn refresh_audio_header_tooltip(ctx: &TransportCtx) {
    audio_tracks::refresh_audio_tooltip_for_player(&ctx.player, &ctx.widgets.vol_menu);
}

/// Non-unified-timeline loads drop the DVD bar cache; unified ones keep it for the seek bar.
fn invalidate_non_dvd_bar_cache(ctx: &Rc<TransportCtx>) {
    if transport_chapter_path_for_ctx(ctx).map_or(true, |p| {
        !crate::playback_entity::PlaybackEntity::resolve(&p).uses_dvd_bar_cache()
    }) {
        *ctx.dvd_bar.borrow_mut() = None;
        sync_seek_chapters(ctx);
    }
}

fn dispatch_file_loaded(ctx: &Rc<TransportCtx>) {
    let chapter_eof = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.take_chapter_eof_load());
    invalidate_non_dvd_bar_cache(ctx);
    reset_media_budgets_on_load(ctx, chapter_eof);
    let recent_visible = ctx.recent_visible.get();
    let browse_hold = recent_visible && !ctx.eof.playback_focus.get();
    log_file_loaded_mode(ctx, browse_hold);
    if browse_hold {
        defer_warm_transport_finish(ctx);
    } else {
        finish_file_loaded_playback(ctx, chapter_eof);
    }
    sync_ui_after_file_loaded(ctx, recent_visible, chapter_eof);
    if !browse_hold {
        schedule_audio_tooltip_refresh(ctx);
    }
}

fn reset_media_budgets_on_load(ctx: &TransportCtx, chapter_eof: bool) {
    if !chapter_eof {
        // Invalidate bundled ME budget fast-path (`vf_smooth_matches_prefs`) so **`apply_mpv_video`**
        // reinstalls vapoursynth: a warm VapourSynth interpreter reused across **`loadfile`** does not adopt
        // a newer ME px² budget (**`RHINO_SMOOTH_MAX_AREA`**) unless **`vf clr`/`vf add`** runs (**`smooth_vf_me_budget_applied`**).
        crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
    }
    crate::video_pref::smooth_budget_reset_session_on_new_media(&ctx.smooth_budget_decoder);
}

fn log_file_loaded_mode(ctx: &TransportCtx, browse_hold: bool) {
    crate::dvd_vob_log::resume_open_log(format!(
        "FileLoaded browse_hold={browse_hold} recent={} focus={}",
        ctx.recent_visible.get(),
        ctx.eof.playback_focus.get()
    ));
}

fn schedule_audio_tooltip_refresh(ctx: &Rc<TransportCtx>) {
    let ctx_tip = Rc::clone(ctx);
    glib::idle_add_local_once(move || refresh_audio_header_tooltip(&ctx_tip));
}

fn sync_ui_after_file_loaded(ctx: &Rc<TransportCtx>, recent_visible: bool, chapter_eof: bool) {
    refresh_dvd_bar_cache_on_idle(ctx);
    sync_window_title_from_context(ctx);
    ctx.eof.sibling_seof.done.set(false);
    ctx.eof.sibling_seof.reset_playback_span();
    sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
    if !recent_visible {
        fit_window_to_video_on_load(ctx);
    }
    refresh_sibling_nav(ctx);
    if !recent_visible {
        tick_and_resync_on_playback_open(ctx);
    }
    finish_file_loaded_ui_sync(ctx, chapter_eof);
}

/// Playback open (grid hidden): fit the window to the incoming video now that its size is known.
fn fit_window_to_video_on_load(ctx: &Rc<TransportCtx>) {
    schedule_window_fit_h_video(
        Rc::clone(&ctx.player),
        ctx.eof.win.clone(),
        ctx.eof.gl.clone(),
    );
}

fn tick_and_resync_on_playback_open(ctx: &Rc<TransportCtx>) {
    transport_tick(ctx);
    schedule_transport_resync_on_idle(ctx);
}

fn finish_file_loaded_ui_sync(ctx: &Rc<TransportCtx>, chapter_eof: bool) {
    refresh_sibling_nav(ctx);
    resync_smooth_unless_resume_pending(ctx, chapter_eof);
    sync_seek_chapters(ctx);
    ctx.blackout.sync();
    crate::video_fill::request_fill_resync();
}

fn refresh_dvd_bar_cache_on_idle(ctx: &Rc<TransportCtx>) {
    let ctx_bar = Rc::clone(ctx);
    glib::idle_add_local_once(move || refresh_dvd_bar_cache(&ctx_bar));
}

fn resync_smooth_unless_resume_pending(ctx: &Rc<TransportCtx>, chapter_eof: bool) {
    if chapter_eof
        || ctx
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.chapter_scrub_resume_pending())
    {
        return;
    }
    schedule_smooth_60_resync_idle(ctx);
}

include!("warm_preload_finish.rs");
include!("file_loaded_playback.rs");
include!("chapter_scrub_resume.rs");
