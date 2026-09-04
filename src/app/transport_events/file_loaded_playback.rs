// Playback-open (grid hidden) completion: resume + track restore and DVD chapter-EOF handling.

fn apply_file_loaded_resume_and_audio(player: &Rc<RefCell<Option<MpvBundle>>>) {
    with_bundle(player, |b| {
        let shell = b.me_budget_shell_path.borrow();
        let shell_ref = shell.as_deref();
        audio_tracks::restore_saved_audio(&b.mpv, shell_ref);
        audio_tracks::ensure_playable_audio(&b.mpv, shell_ref);
        let _ = sub_tracks::restore_saved_sub(&b.mpv, &crate::db::load_sub(), shell_ref);
        // Seek to the resume position *after* selecting the saved track: the audio decoder
        // reopens on `aid` change, so an exact seek that follows re-aligns A/V. Seeking first
        // and switching after left audio drifted on continue until the user nudged the seek bar.
        b.apply_pending_resume();
    });
}

/// Same-title DVD chapter `loadfile` after EOF: rebuild Smooth `vf` after resume seek completes.
fn finish_dvd_chapter_eof_load(ctx: &Rc<TransportCtx>) {
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
    crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
    smooth_60_full_resync_after_media_change(&ctx.player, &ctx.eof.gl, &ctx.eof.reapply_60);
    ctx.eof.gl.queue_render();
}
/// Browse-hold open (continue grid visible): finish the warm load on the next idle so the strip
/// stays responsive, then run resume retries and refresh the audio tooltip.
fn defer_warm_transport_finish(ctx: &Rc<TransportCtx>) {
    let player = Rc::clone(&ctx.player);
    let ctx_warm = Rc::clone(ctx);
    let want_gen = ctx
        .player
        .borrow()
        .as_ref()
        .map(crate::mpv_embed::MpvBundle::warm_file_gen)
        .unwrap_or(0);
    glib::idle_add_local_once(move || {
        warm_preload_finish_load(&player, want_gen);
        schedule_file_resume_retries(&player);
        refresh_audio_header_tooltip(&ctx_warm);
    });
}

/// Playing open (grid hidden): apply resume + audio, finish a DVD chapter-EOF load, and schedule
/// chapter-scrub resume retries when a resume is still pending after the load.
fn finish_file_loaded_playback(ctx: &Rc<TransportCtx>, chapter_eof: bool) {
    apply_file_loaded_resume_and_audio(&ctx.player);
    refresh_audio_header_tooltip(ctx);
    if chapter_eof {
        finish_dvd_chapter_eof_load(ctx);
    }
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending())
    {
        schedule_chapter_scrub_resume_retries(ctx);
    }
}
