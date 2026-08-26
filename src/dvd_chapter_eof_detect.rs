// EOF-tail detection for the open DVD `.vob` (included from `dvd_chapter_eof.rs`).

fn mpv_playback_pos_dur(mpv: &Mpv) -> (f64, f64) {
    let lpos = mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0)
        .unwrap_or(0.0);
    let ldur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    (lpos, ldur)
}

fn ifo_segment_near_eof(ifo_local: f64, ifo_seg: f64) -> bool {
    ifo_seg > 0.0 && (ifo_seg - ifo_local) <= crate::app::TICK_EOF_TAIL_SEC
}

fn chain_head_chapter_context(
    chapter: &Path,
    tl: &DvdVobTimeline,
    mpv_dur: f64,
) -> Option<(usize, f64)> {
    let idx = tl.index_of(chapter)?;
    let seg = tl.chapter_dur_at(idx);
    if crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
        && seg > 0.0
        && chain_head_stretched(mpv_dur, seg)
    {
        Some((idx, seg))
    } else {
        None
    }
}

fn chain_head_ifo_near_eof(
    mpv_pos: f64,
    mpv_dur: f64,
    chapter: &Path,
    tl: &DvdVobTimeline,
) -> bool {
    let Some((_, seg)) = chain_head_chapter_context(chapter, tl, mpv_dur) else {
        return false;
    };
    let ifo = timeline_local_from_mpv(tl, chapter, mpv_pos, mpv_dur);
    ifo_segment_near_eof(ifo, seg)
}

fn chapter_eof_local_sec(mpv: &Mpv, chapter: &Path, tl: &DvdVobTimeline) -> f64 {
    let (lpos, ldur) = mpv_playback_pos_dur(mpv);
    if let Some((_, seg)) = chain_head_chapter_context(chapter, tl, ldur) {
        let ifo = timeline_local_from_mpv(tl, chapter, lpos, ldur);
        return ifo.max((seg - crate::app::TICK_EOF_TAIL_SEC).max(0.0));
    }
    if ldur > 0.0 {
        lpos.max(ldur - crate::app::TICK_EOF_TAIL_SEC)
    } else {
        lpos
    }
}

/// Open chapter near EOF: IFO segment tail on chain-head `.vob`, else mpv `duration` tail.
#[must_use]
pub fn chapter_local_at_eof(mpv: &Mpv) -> bool {
    chapter_local_at_eof_for(mpv, None, None)
}

#[must_use]
pub fn chapter_local_at_eof_for(
    mpv: &Mpv,
    chapter: Option<&Path>,
    tl: Option<&DvdVobTimeline>,
) -> bool {
    let (lpos, ldur) = mpv_playback_pos_dur(mpv);
    if let (Some(ch), Some(tl)) = (chapter, tl) {
        if chain_head_chapter_context(ch, tl, ldur).is_some() {
            return chain_head_ifo_near_eof(lpos, ldur, ch, tl);
        }
    }
    if mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
        return true;
    }
    ldur > 0.0 && (ldur - lpos) <= crate::app::TICK_EOF_TAIL_SEC
}
