fn dvd_sibling_blocked_by_next_chapter(mpv: &libmpv2::Mpv, bar: Option<&DvdBarState>) -> bool {
    let Some(bar) = bar else {
        return false;
    };
    let Some(ch) = open_dvd_chapter_path(mpv, None) else {
        return false;
    };
    bar.tl.next_chapter_after(&ch).is_some()
}

/// True when sibling-folder EOF advance may run (title finished, not mid-`.vob` tail).
pub(crate) fn title_eof_for_sibling_advance(
    mpv: &libmpv2::Mpv,
    bar: Option<&DvdBarState>,
    bar_dur: f64,
    bar_pos: f64,
    core_idle: bool,
) -> bool {
    if dvd_sibling_blocked_by_next_chapter(mpv, bar) {
        return false;
    }
    if mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
        return true;
    }
    // Container `duration` can exceed decoded streams; widen the tail window while stalled.
    let tail_limit = if core_idle {
        crate::media_probe::NEAR_END_SEC
    } else {
        crate::app::TICK_EOF_TAIL_SEC
    };
    if bar_dur > 0.0 {
        let tail = bar_dur - bar_pos;
        if tail > tail_limit {
            return false;
        }
        return true;
    }
    if let Some(bar) = bar {
        if let Some(ch) = open_dvd_chapter_path(mpv, None) {
            return chapter_local_at_eof_for(mpv, Some(ch.as_path()), Some(&bar.tl));
        }
    }
    chapter_local_at_eof(mpv)
}
