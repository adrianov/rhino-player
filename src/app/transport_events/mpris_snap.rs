/// Build an MPRIS property snapshot from transport cache + mpv (Linux shell integration).
fn mpris_shot_from_ctx(ctx: &TransportCtx) -> crate::mpris::MprisShot {
    let (paused, pos, dur) = {
        let c = ctx.cache.borrow();
        (c.pause, c.pos, c.duration)
    };
    let dur = if dur.is_finite() { dur.max(0.0) } else { 0.0 };
    let pos = if pos.is_finite() { pos.max(0.0) } else { 0.0 };

    let mut path_open = false;
    let mut title_tag = None::<String>;
    if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            path_open = crate::mpris::mpv_has_open_path(&b.mpv);
            title_tag = b
                .mpv
                .get_property::<String>("media-title")
                .ok()
                .filter(|s| !s.trim().is_empty());
        }
    }

    let path_res = ctx
        .eof
        .last_path
        .borrow()
        .clone()
        .or_else(|| ctx.player.borrow().as_ref().and_then(|b| crate::media_probe::local_file_from_mpv(&b.mpv)));

    let title = title_tag.or_else(|| {
        path_res.as_ref().and_then(|p| {
            p.file_name().and_then(|s| {
            s.to_str()
                .map(crate::human_media_title::human_media_title)
                    .filter(|t| !t.is_empty())
            })
        })
    });

    let cur = path_res.as_ref().filter(|p| p.is_file());
    let (can_prev, can_next) = if let Some(p) = cur {
        ctx.eof.sibling_seof.nav_sensitivity(p)
    } else {
        (false, false)
    };

    let stopped = !path_open && dur <= f64::EPSILON;

    crate::mpris::MprisShot {
        paused,
        pos_sec: pos,
        dur_sec: dur,
        stopped,
        title,
        track_path: path_res,
        can_prev,
        can_next,
    }
}

fn mpris_enqueue_snapshot(ctx: &TransportCtx) {
    crate::mpris::enqueue_snapshot(mpris_shot_from_ctx(ctx));
}
