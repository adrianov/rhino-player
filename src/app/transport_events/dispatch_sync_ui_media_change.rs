/// Media-lifecycle event handlers: `VideoReconfig`, `path` change, and demux failure.
fn on_video_reconfig(ctx: &Rc<TransportCtx>) {
    sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
    refresh_sibling_nav(ctx);
    transport_tick(ctx);
    sync_seek_chapters(ctx);
    crate::video_fill::request_fill_resync();
    schedule_smooth_60_resync_idle(ctx);
}

fn on_path_changed(ctx: &Rc<TransportCtx>) {
    reset_media_state_for_new_path(ctx);
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

/// Drop per-file caches (video fill, ME budget, DVD bar) before the next media starts.
fn reset_media_state_for_new_path(ctx: &Rc<TransportCtx>) {
    crate::video_fill::request_fill_reset();
    crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
    crate::video_pref::smooth_budget_reset_session_on_new_media(&ctx.smooth_budget_decoder);
    refresh_dvd_bar_cache(ctx);
}

fn dispatch_load_failed(ctx: &Rc<TransportCtx>) {
    // Warm continue-grid preload: stay silent (hover may probe incomplete files).
    if crate::app::browse_overlay_active(&ctx.eof.recent) && !ctx.eof.playback_focus.get() {
        eprintln!("[rhino] load failed during browse warm preload");
        return;
    }
    let path = resolve_failed_media_path(ctx);
    let msg = crate::media_open_fail::message_for_demux_error(path.as_deref());
    log_load_failed(msg, path.as_deref());
    fail_close_current_media(ctx, msg, path.as_deref());
}

/// Remove the continue entry, stop playback, clear state, and surface the failure.
fn fail_close_current_media(ctx: &TransportCtx, msg: &str, path: Option<&std::path::Path>) {
    if let Some(p) = path {
        remove_continue_entry(p);
    }
    if let Some(b) = ctx.player.borrow().as_ref() {
        b.stop_playback();
    }
    *ctx.eof.last_path.borrow_mut() = None;
    (ctx.eof.on_open_fail)(msg.to_string());
}

fn resolve_failed_media_path(ctx: &TransportCtx) -> Option<std::path::PathBuf> {
    ctx.eof.last_path.borrow().clone().or_else(|| {
        ctx.player.borrow().as_ref().and_then(|b| {
            crate::media_probe::shell_media_path(&b.mpv, b.me_budget_shell_path.borrow().as_deref())
        })
    })
}

fn log_load_failed(msg: &str, path: Option<&std::path::Path>) {
    eprintln!(
        "[rhino] load failed (EndFile error): {} ({})",
        msg,
        path.map(|p| p.display().to_string())
            .unwrap_or_else(|| "?".into())
    );
}
