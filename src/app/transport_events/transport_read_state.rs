include!("transport_read_props.rs");
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
    let Some(snap) = crate::media_probe::continue_grid_cache_lookup(&ctx.continue_grid_cache, p)
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

/// Resolve the browse-overlay chapter path and the playing chapter path for the current bundle.
fn resolve_transport_chapters(
    ctx: &TransportCtx,
    b: &MpvBundle,
) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    let shell = b.me_budget_shell_path.borrow().clone();
    let browse_chapter = crate::playback_entity::transport_chapter_path(
        ctx.recent_visible.get(),
        ctx.eof.last_path.borrow().clone(),
        Some(&b.mpv),
        shell.as_deref(),
    );
    let playback_chapter =
        crate::playback_entity::transport_chapter_path(false, None, Some(&b.mpv), shell.as_deref());
    (browse_chapter, playback_chapter)
}

fn persist_transport_bar_if_due(
    ctx: &TransportCtx,
    b: &MpvBundle,
    unified_timeline: bool,
    pos_from_entity_snap: bool,
    dur: f64,
    pos: f64,
) {
    let browse_overlay = ctx.recent_visible.get() && !ctx.eof.playback_focus.get();
    if unified_timeline
        && dur > 0.0
        && !pos_from_entity_snap
        && !browse_overlay
        && !b.resume_seek_pending()
    {
        b.set_transport_bar_persist(dur, pos);
    }
}

fn read_transport_state(ctx: &TransportCtx) -> Option<(bool, bool, f64, f64)> {
    let mut g = ctx.player.try_borrow_mut().ok()?;
    let b = g.as_mut()?;
    let (pause, core_idle, dur, pos) = sample_transport_state(ctx, b);
    let (browse_chapter, playback_chapter) = resolve_transport_chapters(ctx, b);
    let (mut pos, mut dur, pos_from_entity_snap) =
        browse_pause_snap(ctx, &browse_chapter, pause, pos, dur);
    apply_entity_transport_bar(
        ctx,
        b,
        playback_chapter.as_deref(),
        browse_chapter.as_deref(),
        pos_from_entity_snap,
        &mut pos,
        &mut dur,
    );
    Some((pause, core_idle, dur, pos))
}

/// Raw mpv sample + tail clamps + sibling EOF bookkeeping.
fn sample_transport_state(ctx: &TransportCtx, b: &MpvBundle) -> (bool, bool, f64, f64) {
    let (pause, core_idle, eof_reached, pos, raw_dur) = read_mpv_transport_props(b);
    if !b.resume_seek_pending() {
        ctx.eof.sibling_seof.note_transport_pos(pos);
    }
    let played_into_tail = ctx.eof.sibling_seof.played_into_tail(raw_dur, eof_reached);
    let dur =
        duration_clamp_stalled_playout(raw_dur, pos, core_idle, eof_reached, played_into_tail);
    (pause, core_idle, dur, pos)
}

/// Overlay DVD unified-timeline bar state onto the raw mpv position / duration.
fn apply_entity_transport_bar(
    ctx: &TransportCtx,
    b: &mut MpvBundle,
    playback_chapter: Option<&std::path::Path>,
    browse_chapter: Option<&std::path::Path>,
    pos_from_entity_snap: bool,
    pos: &mut f64,
    dur: &mut f64,
) {
    let Some(ch) = playback_chapter.or(browse_chapter) else {
        return;
    };
    let entity = crate::playback_entity::PlaybackEntity::resolve(ch);
    let unified_timeline = entity.has_unified_timeline();
    if unified_timeline {
        *dur = crate::dvd_vob_timeline::clamp_vob_duration(*dur);
    }
    overlay_dvd_bar_state(
        ctx,
        &EntityBarOverlay {
            b,
            playback_chapter,
            browse_chapter,
            entity: &entity,
            pos_from_entity_snap,
        },
        pos,
        dur,
    );
    persist_transport_bar_if_due(ctx, b, unified_timeline, pos_from_entity_snap, *dur, *pos);
}

/// Chapter / snapshot inputs for the DVD unified-timeline bar overlay.
struct EntityBarOverlay<'a> {
    b: &'a MpvBundle,
    playback_chapter: Option<&'a std::path::Path>,
    browse_chapter: Option<&'a std::path::Path>,
    entity: &'a crate::playback_entity::PlaybackEntity,
    pos_from_entity_snap: bool,
}

fn overlay_dvd_bar_state(ctx: &TransportCtx, o: &EntityBarOverlay, pos: &mut f64, dur: &mut f64) {
    let bar = ctx.dvd_bar.borrow();
    if o.pos_from_entity_snap {
        apply_snap_bar_override(o.playback_chapter, bar.as_ref(), o.entity, dur);
    } else if let Some(pb) = o.playback_chapter.or(o.browse_chapter) {
        (*dur, *pos) = o
            .entity
            .transport_bar(pb, *pos, *dur, bar.as_ref(), Some(o.b));
    }
}

fn apply_snap_bar_override(
    playback_chapter: Option<&std::path::Path>,
    bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    entity: &crate::playback_entity::PlaybackEntity,
    dur: &mut f64,
) {
    if let (Some(pb), Some(bar)) = (playback_chapter, bar) {
        if entity.dvd_bar_active(pb, bar) {
            *dur = bar.total_sec();
        }
    }
}
