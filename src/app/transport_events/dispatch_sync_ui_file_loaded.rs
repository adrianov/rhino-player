const CHAPTER_SCRUB_RESUME_RETRY_MS: &[u64] = &[0, 40, 80, 120, 200, 320, 500, 800];
const FILE_RESUME_RETRY_MS: &[u64] = &[40, 80, 120, 200, 320, 500, 800, 1200];

fn schedule_file_resume_retries(player: &Rc<RefCell<Option<MpvBundle>>>) {
    if !player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending())
    {
        return;
    }
    crate::dvd_vob_log::resume_open_log("schedule file resume retries");
    for &ms in FILE_RESUME_RETRY_MS {
        let p = Rc::clone(player);
        let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
            if let Some(b) = p.borrow().as_ref() {
                if b.resume_seek_pending() {
                    b.apply_pending_resume();
                }
            }
        });
    }
}

fn refresh_audio_header_tooltip(ctx: &TransportCtx) {
    audio_tracks::refresh_audio_tooltip_for_player(&ctx.player, &ctx.widgets.vol_menu);
}

fn finish_chapter_scrub_load(ctx: &Rc<TransportCtx>) {
    with_bundle(&ctx.player, |b| {
        let shell = b.me_budget_shell_path.borrow();
        audio_tracks::reapply_after_chapter_load(&b.mpv, shell.as_deref());
    });
    refresh_audio_header_tooltip(ctx);
    schedule_smooth_60_resync_idle(ctx);
    transport_tick(ctx);
    refresh_play_button(ctx);
}

fn try_apply_pending_resume(ctx: &Rc<TransportCtx>) {
    let was_pending = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending());
    with_bundle(&ctx.player, |b| {
        b.apply_pending_resume();
    });
    let still_pending = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending());
    if still_pending {
        schedule_chapter_scrub_resume_retries(ctx);
    } else if was_pending {
        finish_chapter_scrub_load(ctx);
    }
}

fn schedule_chapter_scrub_resume_retries(ctx: &Rc<TransportCtx>) {
    let last = CHAPTER_SCRUB_RESUME_RETRY_MS
        .last()
        .copied()
        .unwrap_or(0);
    for &ms in CHAPTER_SCRUB_RESUME_RETRY_MS {
        let c = Rc::clone(ctx);
        let is_last = ms == last;
        let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
            let was_pending = c
                .player
                .borrow()
                .as_ref()
                .is_some_and(|b| b.chapter_scrub_resume_pending());
            if !was_pending && !is_last {
                return;
            }
            if !was_pending
                && !c
                    .player
                    .borrow()
                    .as_ref()
                    .is_some_and(|b| b.chapter_cross_load_busy())
            {
                return;
            }
            with_bundle(&c.player, |b| {
                b.apply_pending_resume();
                if is_last {
                    b.force_finish_chapter_scrub_playback();
                }
            });
            if is_last || was_pending {
                let still_pending = c
                    .player
                    .borrow()
                    .as_ref()
                    .is_some_and(|b| b.chapter_scrub_resume_pending());
                if (was_pending && !still_pending) || is_last {
                    finish_chapter_scrub_load(&c);
                }
            }
            c.eof.gl.queue_render();
        });
    }
}

fn apply_file_loaded_resume_and_audio(player: &Rc<RefCell<Option<MpvBundle>>>) {
    with_bundle(player, |b| {
        let shell = b.me_budget_shell_path.borrow();
        let shell_ref = shell.as_deref();
        audio_tracks::restore_saved_audio(&b.mpv, shell_ref);
        audio_tracks::ensure_playable_audio(&b.mpv, shell_ref);
        let pr = crate::db::load_sub();
        let _ = sub_tracks::restore_saved_sub(&b.mpv, &pr, shell_ref);
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
fn defer_warm_preload_finish(ctx: &Rc<TransportCtx>) {
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

fn dispatch_file_loaded(ctx: &Rc<TransportCtx>) {
    let chapter_eof = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.take_chapter_eof_load());
    if transport_chapter_path_for_ctx(ctx).is_none_or(|p| {
        !crate::playback_entity::PlaybackEntity::resolve(&p).uses_dvd_bar_cache()
    }) {
        *ctx.dvd_bar.borrow_mut() = None;
        sync_seek_chapters(ctx);
    }
    if !chapter_eof {
        // Invalidate bundled ME budget fast-path (`vf_smooth_matches_prefs`) so **`apply_mpv_video`**
        // reinstalls vapoursynth: a warm VapourSynth interpreter reused across **`loadfile`** does not adopt
        // a newer ME px² budget (**`RHINO_SMOOTH_MAX_AREA`**) unless **`vf clr`/`vf add`** runs (**`smooth_vf_me_budget_applied`**).
        crate::video_pref::forget_bundled_me_budget_vf_apply_on_new_media();
    }
    crate::video_pref::smooth_budget_reset_session_on_new_media(&ctx.smooth_budget_decoder);
    // New file: apply the SQLite-driven resume, restore the saved audio track *before*
    // any unpause so mpv does not play the default `aid` for a fraction of a second and
    // then switch (audio path re-open caused lip-sync drift on continue-grid → reopen).
    // Warm preload behind the continue grid: defer the seek to the next idle so the continue strip stays responsive.
    // Card open sets playback_focus before loadfile — treat that as playback, not browse warm hold.
    let browse_hold = ctx.recent_visible.get() && !ctx.eof.playback_focus.get();
    crate::dvd_vob_log::resume_open_log(format!(
        "FileLoaded browse_hold={browse_hold} recent={} focus={}",
        ctx.recent_visible.get(),
        ctx.eof.playback_focus.get()
    ));
    if browse_hold {
        defer_warm_preload_finish(ctx);
    } else {
        finish_file_loaded_playback(ctx, chapter_eof);
    }
    let ctx_bar = Rc::clone(ctx);
    glib::idle_add_local_once(move || refresh_dvd_bar_cache(&ctx_bar));
    sync_window_title_from_context(ctx);
    ctx.eof.sibling_seof.done.set(false);
    ctx.eof.sibling_seof.reset_playback_span();
    sync_window_aspect_from_player(&ctx.player, &ctx.eof.win_aspect);
    if !ctx.recent_visible.get() {
        schedule_window_fit_h_video(
            Rc::clone(&ctx.player),
            ctx.eof.win.clone(),
            ctx.eof.gl.clone(),
        );
    }
    refresh_sibling_nav(ctx);
    if !ctx.recent_visible.get() {
        transport_tick(ctx);
        schedule_transport_resync_on_idle(ctx);
    }
    if !chapter_eof
        && !ctx
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.chapter_scrub_resume_pending())
    {
        schedule_smooth_60_resync_idle(ctx);
    }
    sync_seek_chapters(ctx);
    ctx.blackout.sync();
    crate::video_fill::request_fill_resync();
    if !browse_hold {
        let ctx_tip = Rc::clone(ctx);
        glib::idle_add_local_once(move || refresh_audio_header_tooltip(&ctx_tip));
    }
}
