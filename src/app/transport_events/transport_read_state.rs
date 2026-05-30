fn browse_pause_snap(
    ctx: &TransportCtx,
    shell: &Option<std::path::PathBuf>,
    pause: bool,
    pos: f64,
    dur: f64,
) -> (f64, f64, bool) {
    if !ctx.recent_visible.get() {
        return (pos, dur, false);
    }
    let Some(p) = shell.as_ref() else {
        return (pos, dur, false);
    };
    let Some(snap) =
        crate::media_probe::continue_grid_cache_lookup(&ctx.continue_grid_cache, p)
    else {
        return (pos, dur, false);
    };
    let mut pos = pos;
    let mut dur = dur;
    let mut from_entity = false;
    if pause {
        pos = snap.resume_sec;
        from_entity = true;
    }
    if dur <= 0.0 {
        dur = snap.duration_sec;
    }
    (pos, dur, from_entity)
}

fn clamp_mpv_sec(v: f64) -> f64 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

/// Container `duration` can exceed the decoded tail; clamp for transport UI when stalled near end.
fn duration_clamp_stalled_playout(
    dur: f64,
    pos: f64,
    core_idle: bool,
    eof_reached: bool,
    played_into_tail: bool,
) -> f64 {
    if dur <= 0.0 || pos <= 0.0 {
        return dur;
    }
    let gap = dur - pos;
    if gap > 0.0
        && gap <= crate::media_probe::NEAR_END_SEC
        && (eof_reached || (core_idle && played_into_tail))
    {
        pos
    } else {
        dur
    }
}

fn read_transport_state(ctx: &TransportCtx) -> Option<(bool, bool, f64, f64)> {
    let mut g = ctx.player.try_borrow_mut().ok()?;
    let b = g.as_mut()?;
    let pause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    let core_idle = b.mpv.get_property::<bool>("core-idle").unwrap_or(false);
    let eof_reached = b.mpv.get_property::<bool>("eof-reached").unwrap_or(false);
    let pos = clamp_mpv_sec(b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0));
    let raw_dur = clamp_mpv_sec(b.mpv.get_property::<f64>("duration").unwrap_or(0.0));
    if !b.resume_seek_pending() {
        ctx.eof.sibling_seof.note_transport_pos(pos);
    }
    let played_into_tail = ctx
        .eof
        .sibling_seof
        .played_into_tail(raw_dur, eof_reached);
    let dur = duration_clamp_stalled_playout(
        raw_dur,
        pos,
        core_idle,
        eof_reached,
        played_into_tail,
    );
    let shell = b.me_budget_shell_path.borrow().clone();
    let browse_chapter = crate::playback_entity::transport_chapter_path(
        ctx.recent_visible.get(),
        ctx.eof.last_path.borrow().clone(),
        Some(&b.mpv),
        shell.as_deref(),
    );
    let playback_chapter = crate::playback_entity::transport_chapter_path(
        false,
        None,
        Some(&b.mpv),
        shell.as_deref(),
    );
    let (mut pos, mut dur, pos_from_entity_snap) =
        browse_pause_snap(ctx, &browse_chapter, pause, pos, dur);
    let bar_chapter = playback_chapter.as_ref().or(browse_chapter.as_ref());
    if let Some(ch) = bar_chapter {
        let entity = crate::playback_entity::PlaybackEntity::resolve(ch);
        if entity.has_unified_timeline() {
            dur = crate::dvd_vob_timeline::clamp_vob_duration(dur);
        }
        let bar = ctx.dvd_bar.borrow();
        if pos_from_entity_snap {
            if let (Some(pb), Some(bar)) = (playback_chapter.as_ref(), bar.as_ref()) {
                if entity.dvd_bar_active(pb, bar) {
                    dur = bar.total_sec();
                }
            }
        } else if let Some(pb) = playback_chapter.as_ref().or(browse_chapter.as_ref()) {
            (dur, pos) = entity.transport_bar(pb, pos, dur, bar.as_ref(), Some(b));
        }
        let browse_overlay = ctx.recent_visible.get() && !ctx.eof.playback_focus.get();
        if entity.has_unified_timeline()
            && dur > 0.0
            && !pos_from_entity_snap
            && !browse_overlay
            && !b.resume_seek_pending()
        {
            b.set_transport_bar_persist(dur, pos);
        }
    }
    Some((pause, core_idle, dur, pos))
}
