// Entity-only DVD segment lengths (included from `dvd_vob_timeline.rs`).

impl DvdVobTimeline {
    /// Whole-title duration stored on the disc entity row (no per-chapter rows).
    fn entity_total_for_split(
        &self,
        chapter: &Path,
        dur_by_path: &HashMap<String, f64>,
    ) -> Option<f64> {
        let per_vob_sum: f64 = self
            .vobs
            .iter()
            .map(|p| dur_from_map(dur_by_path, p))
            .sum();
        if per_vob_sum > 0.0 {
            return None;
        }
        let disc = crate::video_ext::dvd_disc_root(chapter)?;
        let entity_total = dur_from_map(dur_by_path, &disc);
        (entity_total > 0.0).then_some(entity_total)
    }

    /// Allocate segment lengths from the entity total, proportional to `.vob` file sizes.
    fn split_entity_total_by_bytes(&mut self, entity_total: f64) {
        if !(entity_total.is_finite() && entity_total > 0.0) {
            return;
        }
        if self.vobs.len() <= 1 {
            self.durs[0] = entity_total;
            self.recompute_starts();
            return;
        }
        let bytes: Vec<u64> = self
            .vobs
            .iter()
            .map(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
            .collect();
        let sum_b: u64 = bytes.iter().sum();
        if sum_b == 0 {
            return;
        }
        for (i, &b) in bytes.iter().enumerate() {
            let est = entity_total * (b as f64 / sum_b as f64);
            if plausible_vob_dur(est) {
                self.durs[i] = est;
            }
        }
        self.recompute_starts();
    }
}
