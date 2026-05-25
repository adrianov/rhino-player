// Chain-head DVD `.vob` transport bar sync (included from `dvd_vob_timeline.rs`).

/// Chain-head title `.vob`: anchor IFO-local mpv to the title-wide bar while virtual tail is offset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DvdChainBarSync {
    pub anchor_local: f64,
    pub anchor_global: f64,
    pub anchor_playback: f64,
}

impl DvdChainBarSync {
    /// Anchor bar sync to the intended IFO-local target (not transient mpv `time-pos`).
    pub(crate) fn from_targets(ifo_local: f64, hold_global: f64, anchor_playback: f64) -> Self {
        Self {
            anchor_local: ifo_local,
            anchor_global: hold_global,
            anchor_playback,
        }
    }

    pub(crate) fn from_scrub(
        b: &crate::mpv_embed::MpvBundle,
        hold_global: f64,
        ifo_local: f64,
    ) -> Self {
        let anchor_playback = b
            .mpv
            .get_property::<f64>("playback-time")
            .ok()
            .filter(|t| t.is_finite() && *t >= 0.0)
            .unwrap_or(0.0);
        Self::from_targets(ifo_local, hold_global, anchor_playback)
    }

    #[must_use]
    pub(crate) fn global_from_ifo_local(&self, ifo_local: f64, playback: f64, total: f64) -> f64 {
        let delta = if (ifo_local - self.anchor_local).abs() > 0.15 {
            ifo_local - self.anchor_local
        } else {
            (playback - self.anchor_playback).max(0.0)
        };
        (self.anchor_global + delta).clamp(0.0, total.max(0.0))
    }
}

impl DvdBarState {
    /// Title-wide seek-bar position: honor [MpvBundle::dvd_hold_global] only while it matches live time.
    #[must_use]
    pub fn transport_global_pos(
        &self,
        b: &crate::mpv_embed::MpvBundle,
        chapter: &Path,
        local_pos: f64,
    ) -> f64 {
        let raw_local = local_pos.max(0.0);
        if let Some(g) = self.chain_stretch_global(b, chapter, raw_local) {
            return g;
        }
        let implausible = !self.tl.ifo_segment_local_plausible(chapter, raw_local);
        if implausible {
            if let Some(h) = b.dvd_hold_global.get() {
                return h;
            }
            if let Some(sync) = b.dvd_chain_bar_sync.get() {
                let playback = b
                    .mpv
                    .get_property::<f64>("playback-time")
                    .ok()
                    .filter(|t| t.is_finite() && *t >= 0.0)
                    .unwrap_or(sync.anchor_playback);
                let ifo = self.chain_ifo_local(b, chapter, raw_local);
                return sync.global_from_ifo_local(ifo, playback, self.total_sec());
            }
            if let Some(ifo) = self.chain_ifo_local_opt(b, chapter, raw_local) {
                return self.global_pos(chapter, ifo);
            }
            return self.global_pos(chapter, 0.0);
        }
        let computed = self.global_pos(chapter, raw_local);
        match b.dvd_hold_global.get() {
            Some(h) if b.chapter_cross_load_busy() => h,
            Some(h) if (h - computed).abs() <= crate::app::TICK_EOF_TAIL_SEC => h,
            Some(h)
                if crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
                    && !self.tl.ifo_segment_local_plausible(chapter, raw_local) =>
            {
                h
            }
            Some(_) => {
                b.dvd_hold_global.set(None);
                computed
            }
            None => computed,
        }
    }

    fn chain_ifo_local_opt(
        &self,
        b: &crate::mpv_embed::MpvBundle,
        chapter: &Path,
        raw_local: f64,
    ) -> Option<f64> {
        let idx = self.tl.index_of(chapter)?;
        let seg = self.chapter_dur_at(idx);
        if seg <= 0.0 || !crate::dvd_vob_mpv_probe::is_title_chain_head(chapter) {
            return None;
        }
        let mpv_dur = b
            .mpv
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)?;
        if !chain_head_stretched(mpv_dur, seg) {
            return None;
        }
        Some(chain_head_ifo_local_from_mpv(raw_local, mpv_dur, seg))
    }

    fn chain_ifo_local(
        &self,
        b: &crate::mpv_embed::MpvBundle,
        chapter: &Path,
        raw_local: f64,
    ) -> f64 {
        self.chain_ifo_local_opt(b, chapter, raw_local)
            .unwrap_or(raw_local)
    }

    fn chain_stretch_global(
        &self,
        b: &crate::mpv_embed::MpvBundle,
        chapter: &Path,
        raw_local: f64,
    ) -> Option<f64> {
        let ifo = self.chain_ifo_local_opt(b, chapter, raw_local)?;
        if b.chapter_cross_load_busy() {
            return b.dvd_hold_global.get();
        }
        if let Some(sync) = b.dvd_chain_bar_sync.get() {
            let playback = b
                .mpv
                .get_property::<f64>("playback-time")
                .ok()
                .filter(|t| t.is_finite() && *t >= 0.0)
                .unwrap_or(sync.anchor_playback);
            return Some(sync.global_from_ifo_local(ifo, playback, self.total_sec()));
        }
        if let Some(h) = b.dvd_hold_global.get() {
            return Some(h);
        }
        Some(self.global_pos(chapter, ifo))
    }
}
