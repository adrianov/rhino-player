const CHAPTER_SCRUB_RESUME_RETRY_MS: &[u64] = &[0, 40, 80, 120, 200, 320, 500, 800];

fn finish_chapter_scrub_load(ctx: &Rc<TransportCtx>) {
    with_bundle(&ctx.player, |b| {
        audio_tracks::reapply_after_chapter_load(&b.mpv);
    });
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
        b.apply_pending_resume();
        audio_tracks::restore_saved_audio(&b.mpv);
        audio_tracks::ensure_playable_audio(&b.mpv);
        let pr = crate::db::load_sub();
        let _ = sub_tracks::restore_saved_sub(&b.mpv, &pr);
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

fn sync_seek_chapters(ctx: &Rc<TransportCtx>) {
    let mut list = Vec::new();
    if let Some(bar) = ctx.dvd_bar.borrow().as_ref() {
        list = bar.chapter_preview_labels();
    } else if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            list = crate::chapter_list::mpv_chapter_list(&b.mpv);
        }
    }
    *ctx.seek_chapters.borrow_mut() = list;
}

fn dvd_bar_duration(ctx: &TransportCtx) -> Option<f64> {
    let chapter = transport_chapter_path_for_ctx(ctx)?;
    let bar = ctx.dvd_bar.borrow();
    let bar = bar.as_ref()?;
    crate::playback_entity::PlaybackEntity::resolve(&chapter)
        .transport_duration_from_bar(&chapter, bar)
}

fn transport_chapter_path_for_ctx(ctx: &TransportCtx) -> Option<std::path::PathBuf> {
    if ctx.recent_visible.get() {
        return ctx.eof.last_path.borrow().clone();
    }
    let g = ctx.player.try_borrow().ok()?;
    let b = g.as_ref()?;
    let shell = b.me_budget_shell_path.borrow().clone();
    crate::playback_entity::transport_chapter_path(false, None, Some(&b.mpv), shell.as_deref())
}

/// Refresh window / header title when mpv `path` changes (sibling DVD advance, etc.).
fn sync_window_title_from_context(ctx: &Rc<TransportCtx>) {
    if ctx.recent_visible.get() {
        return;
    }
    let path = ctx
        .player
        .borrow()
        .as_ref()
        .and_then(|b| {
            crate::media_probe::shell_media_path(
                &b.mpv,
                b.me_budget_shell_path.borrow().as_deref(),
            )
        })
        .or_else(|| ctx.eof.last_path.borrow().clone());
    let Some(path) = path else {
        return;
    };
    let ttl = crate::playback_entity::window_title_for(&path);
    sync_app_window_title(
        &ctx.eof.win,
        ctx.eof.hdr_title_mirror.as_deref(),
        Some(&ttl),
    );
}

fn refresh_dvd_bar_cache(ctx: &Rc<TransportCtx>) {
    let Ok(g) = ctx.player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        *ctx.dvd_bar.borrow_mut() = None;
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    crate::dvd_vob_timeline::refresh_dvd_bar(&ctx.dvd_bar, &b.mpv, shell.as_deref());
    sync_seek_chapters(ctx);
}

fn maybe_refresh_dvd_bar_cache(ctx: &Rc<TransportCtx>) {
    let Ok(g) = ctx.player.try_borrow() else {
        return;
    };
    let Some(b) = g.as_ref() else {
        return;
    };
    let shell = b.me_budget_shell_path.borrow();
    crate::dvd_vob_timeline::maybe_refresh_dvd_bar(&ctx.dvd_bar, &b.mpv, shell.as_deref());
    sync_seek_chapters(ctx);
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
    // Warm preload: defer the seek to the next idle so the continue strip stays responsive.
    if ctx.recent_visible.get() {
        let player = Rc::clone(&ctx.player);
        let want_gen = ctx
            .player
            .borrow()
            .as_ref()
            .map(crate::mpv_embed::MpvBundle::warm_file_gen)
            .unwrap_or(0);
        glib::idle_add_local_once(move || {
            warm_preload_finish_load(&player, want_gen);
        });
    } else {
        apply_file_loaded_resume_and_audio(&ctx.player);
        if chapter_eof {
            finish_dvd_chapter_eof_load(ctx);
            if ctx
                .player
                .borrow()
                .as_ref()
                .is_some_and(|b| b.chapter_scrub_resume_pending())
            {
                schedule_chapter_scrub_resume_retries(ctx);
            }
        } else if ctx
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.chapter_scrub_resume_pending())
        {
            schedule_chapter_scrub_resume_retries(ctx);
        }
    }
    let ctx_bar = Rc::clone(ctx);
    glib::idle_add_local_once(move || refresh_dvd_bar_cache(&ctx_bar));
    sync_window_title_from_context(ctx);
    ctx.eof.sibling_seof.done.set(false);
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
}
