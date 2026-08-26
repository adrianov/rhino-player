/// Build an MPRIS property snapshot from transport cache + mpv (Linux shell integration).
fn mpris_shot_from_ctx(ctx: &TransportCtx) -> crate::mpris::MprisShot {
    let (paused, pos, dur) = cached_playback_snapshot(ctx);

    let (path_open, title_tag) = mpris_player_state(ctx);
    let path_res = mpris_track_path(ctx);
    let title = mpris_title(&title_tag, path_res.as_ref());
    let cur = path_res.as_ref().filter(|p| p.is_file());
    let (can_prev, can_next) = mpris_nav_sensitivity(ctx, cur);

    crate::mpris::MprisShot {
        paused,
        pos_sec: pos,
        dur_sec: dur,
        stopped: !path_open && dur <= f64::EPSILON,
        title,
        track_path: path_res,
        can_prev,
        can_next,
    }
}

/// Cached pause / position / duration from the last transport tick, clamped to sane ranges.
fn cached_playback_snapshot(ctx: &TransportCtx) -> (bool, f64, f64) {
    let (paused, pos, dur) = {
        let c = ctx.cache.borrow();
        (c.pause, c.pos, c.duration)
    };
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };
    (paused, pos, dur)
}

/// Open-path flag + mpv media-title tag from the live bundle, if any.
fn mpris_player_state(ctx: &TransportCtx) -> (bool, Option<String>) {
    if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            let path_open = crate::mpris::mpv_has_open_path(&b.mpv);
            let title_tag = b
                .mpv
                .get_property::<String>("media-title")
                .ok()
                .filter(|s| !s.trim().is_empty());
            return (path_open, title_tag);
        }
    }
    (false, None)
}

fn mpris_track_path(ctx: &TransportCtx) -> Option<std::path::PathBuf> {
    ctx.eof.last_path.borrow().clone().or_else(|| {
        ctx.player
            .borrow()
            .as_ref()
            .and_then(|b| crate::media_probe::local_file_from_mpv(&b.mpv))
    })
}

fn mpris_title(title_tag: &Option<String>, path: Option<&std::path::PathBuf>) -> Option<String> {
    title_tag.clone().or_else(|| {
        path.and_then(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(crate::human_media_title::human_media_title)
                .filter(|t| !t.is_empty())
        })
    })
}

fn mpris_nav_sensitivity(ctx: &TransportCtx, cur: Option<&std::path::PathBuf>) -> (bool, bool) {
    if let Some(p) = cur {
        ctx.eof.sibling_seof.nav_sensitivity(p)
    } else {
        (false, false)
    }
}

fn mpris_enqueue_snapshot(ctx: &TransportCtx) {
    crate::mpris::enqueue_snapshot(mpris_shot_from_ctx(ctx));
}
