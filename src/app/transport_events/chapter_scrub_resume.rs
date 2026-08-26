// Chapter-scrub resume: retry the stashed seek across `loadfile` settle, then finish
// the chapter load (audio reapply + Smooth resync + transport refresh).

const CHAPTER_SCRUB_RESUME_RETRY_MS: &[u64] = &[0, 40, 80, 120, 200, 320, 500, 800];

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
    let last = CHAPTER_SCRUB_RESUME_RETRY_MS.last().copied().unwrap_or(0);
    for &ms in CHAPTER_SCRUB_RESUME_RETRY_MS {
        let c = Rc::clone(ctx);
        let is_last = ms == last;
        let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
            chapter_scrub_retry_once(c.clone(), is_last);
        });
    }
}

fn chapter_scrub_retry_once(c: Rc<TransportCtx>, is_last: bool) {
    let Some(was_pending) = chapter_retry_gate(&c, is_last) else {
        return;
    };
    with_bundle(&c.player, |b| {
        b.apply_pending_resume();
        if is_last {
            b.force_finish_chapter_scrub_playback();
        }
    });
    let still_pending = c
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending());
    if (was_pending && !still_pending) || is_last {
        finish_chapter_scrub_load(&c);
    }
    c.eof.gl.queue_render();
}

/// Decides whether this retry tick acts at all; `Some` carries the pending state at entry.
fn chapter_retry_gate(c: &TransportCtx, is_last: bool) -> Option<bool> {
    let was_pending = c
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending());
    if !was_pending && !is_last {
        return None;
    }
    if !was_pending
        && !c
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.chapter_cross_load_busy())
    {
        return None;
    }
    Some(was_pending)
}
