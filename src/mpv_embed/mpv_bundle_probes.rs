// Shared `MpvBundle` probes: open-shell identity (DVD title chain heads) and mpv duration
// reads. Included at module level so every resume / persistence / chapter-scrub extension
// sees the same helpers instead of repeating borrow-and-filter chains.

impl MpvBundle {
    /// Open shell path is a DVD title chain-head `.vob` (chapter-local coordinates).
    fn open_shell_is_chain_head(&self) -> bool {
        self.me_budget_shell_path
            .borrow()
            .as_ref()
            .is_some_and(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
    }

    /// Cloned open shell path when it is a DVD title chain-head `.vob`.
    fn chain_head_shell_path(&self) -> Option<std::path::PathBuf> {
        self.me_budget_shell_path
            .borrow()
            .clone()
            .filter(|p| crate::dvd_vob_mpv_probe::is_title_chain_head(p))
    }

    /// True when mpv already sits at `t`: chain-head IFO coords when the open media is a
    /// chain head, plain seconds otherwise.
    fn resume_at_target(&self, t: f64) -> bool {
        match &self.chain_head_shell_path() {
            Some(path) => resume_seek::resume_already_at_ifo(&self.mpv, path, t),
            None => resume_seek::resume_already_at(&self.mpv, t),
        }
    }

    /// Positive finite mpv **`duration`** (0 until demux publishes one).
    fn finite_positive_duration(&self) -> f64 {
        self.mpv
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite() && *d > 0.0)
            .unwrap_or(0.0)
    }

    /// Finite mpv **`duration`** as last known (may be 0 until demux runs); 0 when unreadable.
    fn finite_duration_secs(&self) -> f64 {
        self.mpv
            .get_property::<f64>("duration")
            .ok()
            .filter(|d| d.is_finite())
            .unwrap_or(0.0)
    }
    /// Finite mpv seconds for `prop`, clamped at 0; None when unreadable.
    fn finite_mpv_secs_nonneg(&self, prop: &str) -> Option<f64> {
        self.mpv
            .get_property::<f64>(prop)
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| v.max(0.0))
    }
}

/// Canonicalized path, or the input unchanged when canonicalization fails.
fn canonicalize_media_path(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Entity-row duration from the SQLite media map, when known and positive.
fn entity_row_duration(
    key: &std::path::Path,
    map: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    key.to_str()
        .and_then(|k| map.get(k).copied())
        .filter(|d| d.is_finite() && *d > 0.0)
}
