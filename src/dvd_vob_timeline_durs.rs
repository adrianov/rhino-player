impl DvdVobTimeline {
    /// Authoritative segment lengths from `VTS_xx_0.IFO` cell playback times.
    pub(crate) fn apply_ifo_chapter_durs(&mut self, chapter: &Path) {
        let Some(ifo_durs) = crate::dvd_ifo_parse::title_vob_durations(chapter) else {
            return;
        };
        if ifo_durs.len() != self.vobs.len() {
            return;
        }
        for (i, d) in ifo_durs.into_iter().enumerate() {
            if d.is_finite() && d > 0.0 {
                self.durs[i] = d;
            }
        }
        self.recompute_starts();
    }

    /// Prefer mpv `duration` on the open `.vob` when it differs from the stored value.
    pub(crate) fn apply_live_chapter_dur(&mut self, chapter: &Path, live_dur: f64) {
        let live_dur = clamp_vob_duration(live_dur);
        if live_dur <= 0.0 {
            return;
        }
        let Some(i) = self.index_of(chapter) else {
            return;
        };
        if live_dur + 0.5 < self.durs[i] {
            self.durs[i] = live_dur;
        } else {
            self.grow_chapter_dur(chapter, live_dur);
        }
        self.recompute_starts();
    }

    /// Extend one segment when live mpv duration exceeds the stored value.
    pub fn grow_chapter_dur(&mut self, chapter: &Path, live_dur: f64) {
        let live_dur = clamp_vob_duration(live_dur);
        if live_dur <= 0.0 {
            return;
        }
        let Some(i) = self.index_of(chapter) else {
            return;
        };
        if live_dur <= self.durs[i] + 0.5 {
            return;
        }
        self.durs[i] = live_dur;
        self.recompute_starts();
    }

    /// Clear segment lengths above [MAX_VOB_DUR_SEC] (bad mpv / legacy rows).
    pub(crate) fn scrub_implausible_durs(&mut self) {
        let mut changed = false;
        for d in &mut self.durs {
            if *d > MAX_VOB_DUR_SEC || !d.is_finite() {
                *d = 0.0;
                changed = true;
            }
        }
        if changed {
            self.recompute_starts();
        }
    }

    /// Fill missing segments from the median of known sibling chapter lengths.
    pub(crate) fn infer_missing_from_siblings(&mut self) -> bool {
        let mut known: Vec<f64> = self
            .durs
            .iter()
            .copied()
            .filter(|d| *d > 0.0 && *d <= MAX_VOB_DUR_SEC)
            .collect();
        if known.len() < 2 {
            return false;
        }
        known.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = known[known.len() / 2];
        let mut changed = false;
        for d in &mut self.durs {
            if *d <= 0.0 {
                *d = median;
                changed = true;
            }
        }
        if changed {
            self.recompute_starts();
        }
        changed
    }
}
