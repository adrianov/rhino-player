/// Quiet period after `FileLoaded` / `VideoReconfig` / `path` / `container-fps` before
/// [smooth_60_full_resync_after_media_change]: mpv often emits those in separate drains; one timer
/// coalesces them so the bundled `.vpy` is not built twice with stale `container-fps` or SQLite ME rows.
const SMOOTH_60_RESYNC_DEBOUNCE: Duration = Duration::from_millis(160);

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

fn arm_smooth_60_resync_debounce(ctx: &Rc<TransportCtx>, fire: fn(&Rc<TransportCtx>)) {
    drop_glib_source(ctx.smooth_60_resync_debounce.as_ref());
    let deb = Rc::clone(&ctx.smooth_60_resync_debounce);
    let c = Rc::clone(ctx);
    *ctx.smooth_60_resync_debounce.borrow_mut() = Some(glib::timeout_add_local(
        SMOOTH_60_RESYNC_DEBOUNCE,
        move || {
            *deb.borrow_mut() = None;
            fire(&c);
            glib::ControlFlow::Break
        },
    ));
}

fn smooth_60_resync_fire(ctx: &Rc<TransportCtx>) {
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending())
    {
        // Retry stashed resume (duration may land between debounce ticks) so Smooth-on reload
        // cannot loop forever while pause is held.
        with_bundle(&ctx.player, |b| {
            b.apply_pending_resume();
        });
        if ctx
            .player
            .borrow()
            .as_ref()
            .is_some_and(|b| b.resume_seek_pending())
        {
            eprintln!("[rhino] video: smooth resync deferred (resume seek pending)");
            schedule_smooth_60_resync_idle(ctx);
            return;
        }
    }
    smooth_60_full_resync_after_media_change(&ctx.player, &ctx.eof.gl, &ctx.eof.reapply_60);
}

fn cancel_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    drop_glib_source(ctx.smooth_60_resync_debounce.as_ref());
}

fn schedule_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    if defer_for_recent_grid(ctx) {
        return;
    }
    if defer_while_vf_attach_pending(ctx) {
        return;
    }
    if blocked_by_chapter_scrub_resume(ctx) {
        return;
    }
    if skip_when_smooth_off_and_vf_gone(ctx) {
        return;
    }
    sync_media_decode_row_for_me_budget(&ctx.player);
    arm_smooth_60_resync_debounce(ctx, smooth_60_resync_fire);
}

fn defer_for_recent_grid(ctx: &Rc<TransportCtx>) -> bool {
    if !ctx.recent_visible.get() {
        return false;
    }
    // [transport_drain_after_loadfile] can emit FileLoaded before [reveal_ui_after_load] hides
    // the continue grid on a playback open — retry once recent hides. Browse-only warm preload
    // (grid stays up, no playback focus) keeps the early return.
    if ctx.eof.playback_focus.get() {
        let c = Rc::clone(ctx);
        glib::idle_add_local_once(move || schedule_smooth_60_resync_idle(&c));
    }
    true
}

fn defer_while_vf_attach_pending(ctx: &Rc<TransportCtx>) -> bool {
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.smooth_vf_attach_pending())
    {
        eprintln!("[rhino] video: smooth resync deferred (vapoursynth attach in flight)");
        arm_smooth_60_resync_debounce(ctx, schedule_smooth_60_resync_idle);
        return true;
    }
    false
}

fn blocked_by_chapter_scrub_resume(ctx: &Rc<TransportCtx>) -> bool {
    ctx.player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.chapter_scrub_resume_pending())
}

fn skip_when_smooth_off_and_vf_gone(ctx: &Rc<TransportCtx>) -> bool {
    if ctx.video_pref.borrow().smooth_60 {
        return false;
    }
    ctx.player.borrow().as_ref().map_or(true, |b| {
        if crate::video_pref::vf_chain_has_vapoursynth(&b.mpv) {
            return false;
        }
        // Bob (Blu-ray / local 1080i) still needs apply when Smooth is off.
        !crate::video_pref::bob_needs_apply_when_smooth_off(&b.mpv, Some(b))
    })
}
