// Resume-open probe helpers for `DvdVobTimeline` (included from `dvd_vob_timeline.rs`).

impl DvdVobTimeline {
    /// True when [DvdVobTimeline::resolve_global] can place `global` without probing more segments.
    pub(crate) fn can_resolve_global(&self, global: f64) -> bool {
        if self.vobs.is_empty() {
            return false;
        }
        let g = global.max(0.0);
        let mut pos = 0.0;
        for i in 0..self.vobs.len() {
            let d = self.durs[i];
            if d <= 0.0 {
                return g < pos;
            }
            let end = pos + d;
            if g < end - 1e-3 {
                return true;
            }
            pos = end;
        }
        self.durs
            .last()
            .is_some_and(|d| *d > 0.0 && g <= pos + 1e-3)
    }

    /// Headless probe from the first chapter until `global` can be resolved (resume open).
    pub(crate) fn probe_prefix_for_global(&mut self, global: f64) -> usize {
        let g = global.max(0.0);
        let mut probed = 0;
        while !self.can_resolve_global(g) {
            let Some(i) = self.first_missing_index_for_global(g) else {
                break;
            };
            if let Some(d) = crate::dvd_vob_mpv_probe::probe_vob_duration(&self.vobs[i]) {
                self.durs[i] = d;
                probed += 1;
            } else {
                self.infer_missing_from_siblings();
            }
            self.recompute_starts();
        }
        probed
    }

    fn first_missing_index_for_global(&self, global: f64) -> Option<usize> {
        let mut pos = 0.0;
        for i in 0..self.vobs.len() {
            let d = self.durs[i];
            if d > 0.0 {
                pos += d;
                continue;
            }
            if global >= pos {
                return Some(i);
            }
            return None;
        }
        None
    }
}
