fn natural_eof_for_advance(ctx: &TransportCtx, core_idle: bool) -> bool {
    if core_idle {
        return true;
    }
    let Ok(g) = ctx.player.try_borrow() else {
        return false;
    };
    let Some(b) = g.as_ref() else {
        return false;
    };
    b.mpv.get_property::<bool>("eof-reached").unwrap_or(false)
}

fn sibling_eof_ready(ctx: &TransportCtx, dur: f64, pos: f64, core_idle: bool) -> bool {
    if ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.resume_seek_pending())
    {
        return false;
    }
    let eof_reached = ctx
        .player
        .borrow()
        .as_ref()
        .is_some_and(|b| b.mpv.get_property::<bool>("eof-reached").unwrap_or(false));
    if !ctx.eof.sibling_seof.played_into_tail(dur, eof_reached) {
        return false;
    }
    dvd_eof_tail(ctx, dur, pos, core_idle)
}

fn maybe_advance_dvd_chapter_eof(ctx: &Rc<TransportCtx>) -> bool {
    if crate::app::browse_overlay_active(&ctx.eof.recent) {
        return false;
    }
    {
        let Ok(g) = ctx.player.try_borrow() else {
            return false;
        };
        let Some(b) = g.as_ref() else {
            return false;
        };
        let shell = b.me_budget_shell_path.borrow();
        crate::dvd_vob_timeline::refresh_dvd_bar_at_chapter_eof(
            &ctx.dvd_bar,
            &b.mpv,
            shell.as_deref(),
        );
    }
    let advanced = {
        let bar = ctx.dvd_bar.borrow();
        let Some(ref bar) = *bar else {
            return false;
        };
        crate::dvd_vob_timeline::advance_title_chapter_eof(&ctx.player, bar)
    };
    if !advanced {
        return false;
    }
    crate::app::transport_drain_after_loadfile_idle();
    true
}

fn dvd_eof_tail(ctx: &TransportCtx, bar_dur: f64, bar_pos: f64, core_idle: bool) -> bool {
    let Ok(g) = ctx.player.try_borrow() else {
        return false;
    };
    let Some(b) = g.as_ref() else {
        return false;
    };
    let bar = ctx.dvd_bar.borrow();
    crate::dvd_vob_timeline::title_eof_for_sibling_advance(
        &b.mpv,
        bar.as_ref(),
        bar_dur,
        bar_pos,
        core_idle,
    )
}
