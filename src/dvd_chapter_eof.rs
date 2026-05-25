// DVD mid-title chapter EOF: detect tail of open `.vob` and load the next chapter.

use libmpv2::Mpv;

impl DvdVobTimeline {
    /// Map mpv EOF on the open `.vob` to the next `loadfile` target and whole-title hold time.
    ///
    /// PTT chapter marks and on-disk `.vob` splits rarely align; always derive the target from
    /// live tail `time-pos` / `duration`, not the stored start of the next file in the bar.
    pub fn continue_after_vob_eof(
        &self,
        current: &Path,
        local_eof: f64,
    ) -> Option<(PathBuf, f64, f64)> {
        let i = self.index_of(current)?;
        if i + 1 >= self.vobs.len() {
            return None;
        }
        let local_eof = local_eof.max(0.0);
        let g_eof = self.starts[i] + local_eof;
        let g_cont = (g_eof + 0.05).min(self.total_sec);
        let (idx, mut local) = self.resolve_global(g_cont);
        let mut target = self.vobs.get(idx)?.clone();
        if crate::video_ext::paths_same_file(&target, current) {
            let j = i + 1;
            target = self.vobs[j].clone();
            let stored_end = self.starts[i] + self.durs[i].max(0.0);
            local = if g_eof + 1e-3 >= stored_end {
                (g_cont - self.starts[j]).max(0.0)
            } else {
                0.0
            };
            if self.durs[j] > 0.0 {
                local = local.min((self.durs[j] - 0.05).max(0.0));
            }
        }
        Some((target, local, g_cont))
    }
}

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

fn chain_head_ifo_near_eof(mpv_pos: f64, mpv_dur: f64, chapter: &Path, tl: &DvdVobTimeline) -> bool {
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

/// Load the next chapter in the same DVD title when the open file ends but the title has not.
#[must_use]
pub fn advance_title_chapter_eof(
    player: &std::rc::Rc<std::cell::RefCell<Option<crate::mpv_embed::MpvBundle>>>,
    bar: &DvdBarState,
) -> bool {
    let Ok(mut g) = player.try_borrow_mut() else {
        return false;
    };
    let Some(b) = g.as_mut() else {
        return false;
    };
    let shell = b.me_budget_shell_path.borrow().clone();
    let Some(chapter) = open_dvd_chapter_path(&b.mpv, shell.as_deref()) else {
        return false;
    };
    if !chapter_local_at_eof_for(&b.mpv, Some(chapter.as_path()), Some(&bar.tl)) {
        return false;
    }
    if b.chapter_cross_load_busy() {
        if b.chapter_scrub_resume_pending() {
            return false;
        }
        crate::dvd_vob_log::dvd_seek_log("eof_advance: clear stale chapter scrub");
        b.abort_chapter_load(true);
    }
    let local_eof = chapter_eof_local_sec(&b.mpv, &chapter, &bar.tl);
    let Some((next, local, hold_global)) = bar.tl.continue_after_vob_eof(&chapter, local_eof)
    else {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "eof_advance: no next segment after {} local={local_eof:.2}",
            chapter.display()
        ));
        return false;
    };
    if crate::video_ext::paths_same_file(&next, &chapter) {
        return false;
    }
    crate::dvd_vob_log::dvd_seek_log(format!(
        "eof_advance: {} -> {} global={hold_global:.2} local={local:.2} (tail={local_eof:.2})",
        chapter.display(),
        next.display()
    ));
    if b
        .load_chapter_seek(&next, local, hold_global, true, true)
        .is_err()
    {
        eprintln!(
            "[rhino] dvd: eof_advance loadfile failed {} -> {}",
            chapter.display(),
            next.display()
        );
        return false;
    }
    true
}
