// Chain-head `.vob` seek: mpv virtual timeline vs IFO segment locals (included from `dvd_vob_timeline.rs`).

#[must_use]
pub(crate) fn chain_head_stretched(mpv_dur: f64, ifo_seg: f64) -> bool {
    ifo_seg > 0.0 && mpv_dur > ifo_seg * 1.5
}

#[must_use]
pub(crate) fn chain_head_tail(mpv_dur: f64, ifo_seg: f64) -> f64 {
    (mpv_dur - ifo_seg).max(0.0)
}

fn mpv_duration(mpv: &libmpv2::Mpv) -> f64 {
    mpv.get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0)
}

/// mpv `time-pos` in IFO segment locals (virtual tail at `chain_head_tail`).
#[must_use]
pub(crate) fn chain_head_ifo_local_from_mpv(mpv_pos: f64, mpv_dur: f64, ifo_seg: f64) -> f64 {
    let mpv_pos = mpv_pos.max(0.0);
    if ifo_seg <= 0.0 || !chain_head_stretched(mpv_dur, ifo_seg) {
        return mpv_pos;
    }
    let tail = chain_head_tail(mpv_dur, ifo_seg);
    if mpv_pos >= tail - 0.5 {
        return (mpv_pos - tail).clamp(0.0, ifo_seg);
    }
    mpv_pos.clamp(0.0, ifo_seg)
}

/// IFO-local seconds → mpv `time-pos` (tail + ifo when demuxer reports virtual tail).
#[must_use]
pub(crate) fn chain_head_ifo_local_to_mpv(ifo_local: f64, mpv_dur: f64, ifo_seg: f64, at_tail: bool) -> f64 {
    let ifo = ifo_local.clamp(0.0, ifo_seg.max(0.0));
    if ifo_seg <= 0.0 || !chain_head_stretched(mpv_dur, ifo_seg) {
        return ifo.min((mpv_dur - 0.05).max(ifo));
    }
    if at_tail {
        (chain_head_tail(mpv_dur, ifo_seg) + ifo).min((mpv_dur - 0.05).max(0.0))
    } else {
        ifo.min((ifo_seg - 0.05).max(ifo))
    }
}

/// IFO segment length for a chain-head chapter path (title timeline, not mpv live dur).
pub(crate) fn chain_head_ifo_seg(chapter: &std::path::Path) -> Option<f64> {
    if !crate::dvd_vob_mpv_probe::is_title_chain_head(chapter) {
        return None;
    }
    let map = crate::db::load_duration_map();
    let tl = crate::dvd_entity::build_title_timeline(chapter, &map, 0.0)?;
    let idx = tl.index_of(chapter)?;
    let seg = tl.chapter_dur_at(idx);
    (seg > 0.0).then_some(seg)
}

/// mpv seek target for an IFO-local offset on a chain-head `.vob` (virtual tail + ifo when stretched).
#[must_use]
pub(crate) fn chain_head_mpv_seek_sec(mpv: &libmpv2::Mpv, ifo_local: f64, ifo_seg: f64) -> f64 {
    let mpv_dur = mpv_duration(mpv);
    let ifo = ifo_local.clamp(0.0, ifo_seg.max(0.0));
    chain_head_ifo_local_to_mpv(ifo, mpv_dur, ifo_seg, chain_head_stretched(mpv_dur, ifo_seg))
}

/// IFO-local chapter offset from live mpv coords (resume DB + continue-grid global mapping).
#[must_use]
pub(crate) fn timeline_local_from_mpv(
    tl: &DvdVobTimeline,
    chapter: &Path,
    mpv_pos: f64,
    mpv_dur: f64,
) -> f64 {
    let mpv_pos = if mpv_pos.is_finite() { mpv_pos.max(0.0) } else { 0.0 };
    let Some(idx) = tl.index_of(chapter) else {
        return mpv_pos;
    };
    let seg = tl.chapter_dur_at(idx);
    if crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
        && seg > 0.0
        && chain_head_stretched(mpv_dur, seg)
    {
        return chain_head_ifo_local_from_mpv(mpv_pos, mpv_dur, seg);
    }
    if !tl.ifo_segment_local_plausible(chapter, mpv_pos) {
        return tl.clamp_ifo_segment_local(chapter, mpv_pos);
    }
    mpv_pos
}

/// Preview / continue-grid seek: map IFO-local chapter time to mpv `time-pos` on chain-head `.vob`.
#[must_use]
pub(crate) fn preview_mpv_seek_sec(
    chapter: &std::path::Path,
    ifo_local: f64,
    mpv: &libmpv2::Mpv,
) -> f64 {
    chain_head_ifo_seg(chapter)
        .map(|seg| chain_head_mpv_seek_sec(mpv, ifo_local, seg))
        .unwrap_or(ifo_local)
}

/// True when mpv has reported the virtual tail duration for a chain-head chapter.
#[must_use]
pub(crate) fn chain_head_mpv_ready(chapter: &std::path::Path, mpv: &libmpv2::Mpv) -> bool {
    let Some(seg) = chain_head_ifo_seg(chapter) else {
        return mpv_duration(mpv) > 0.0;
    };
    chain_head_stretched(mpv_duration(mpv), seg)
}
