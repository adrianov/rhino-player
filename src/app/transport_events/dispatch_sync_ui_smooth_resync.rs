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
        eprintln!("[rhino] video: smooth resync deferred (resume seek pending)");
        schedule_smooth_60_resync_idle(ctx);
        return;
    }
    smooth_60_full_resync_after_media_change(&ctx.player, &ctx.eof.gl, &ctx.eof.reapply_60);
}

fn cancel_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    drop_glib_source(ctx.smooth_60_resync_debounce.as_ref());
}

fn schedule_smooth_60_resync_idle(ctx: &Rc<TransportCtx>) {
    if ctx.recent_visible.get() {
        return;
    }
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.smooth_vf_attach_pending())
    {
        eprintln!("[rhino] video: smooth resync deferred (vapoursynth attach in flight)");
        arm_smooth_60_resync_debounce(ctx, schedule_smooth_60_resync_idle);
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
    if !ctx.video_pref.borrow().smooth_60 {
        let vf_gone = ctx
            .player
            .borrow()
            .as_ref()
            .is_none_or(|b| !crate::video_pref::vf_chain_has_vapoursynth(&b.mpv));
        if vf_gone {
            return;
        }
    }
    sync_media_decode_row_for_me_budget(&ctx.player);
    arm_smooth_60_resync_debounce(ctx, smooth_60_resync_fire);
}
