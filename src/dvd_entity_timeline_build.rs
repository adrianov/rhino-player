// How `build_title_timeline_with` may call headless libmpv for missing segment lengths.

/// How [build_title_timeline_with] may call headless libmpv for missing segment lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TimelineBuildOpts {
    pub mpv_probe: bool,
    pub probe_budget: Option<usize>,
}

impl TimelineBuildOpts {
    /// SQLite + live mpv only; missing segment lengths filled later on idle (no headless probe).
    pub const PLAYBACK: Self = Self {
        mpv_probe: false,
        probe_budget: None,
    };
    /// Same flags as [Self::PLAYBACK]; name marks card/still/sanitize reads that must not probe.
    pub const CACHE_ONLY: Self = Self::PLAYBACK;
    pub const BACKGROUND: Self = Self {
        mpv_probe: true,
        probe_budget: Some(crate::dvd_vob_mpv_probe::BG_PROBE_BATCH),
    };
    /// Integration tests: probe every missing segment in the title set.
    #[cfg(test)]
    pub const FULL: Self = Self {
        mpv_probe: true,
        probe_budget: None,
    };
}

/// Unified timeline from on-disk title `.vob` queue and per-file durations only.
pub(crate) fn build_title_timeline(
    chapter: &Path,
    dur_by_path: &HashMap<String, f64>,
    live_local_dur: f64,
) -> Option<DvdVobTimeline> {
    build_title_timeline_with(chapter, dur_by_path, live_local_dur, TimelineBuildOpts::PLAYBACK)
}

pub(crate) fn build_title_timeline_with(
    chapter: &Path,
    dur_by_path: &HashMap<String, f64>,
    live_local_dur: f64,
    opts: TimelineBuildOpts,
) -> Option<DvdVobTimeline> {
    DvdVobTimeline::from_title_vobs_with(chapter, dur_by_path, live_local_dur, opts)
}
