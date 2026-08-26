//! Virtual DVD timeline: queued `.vob` files for one title set and cumulative timing.
//! See `docs/features/30-dvd-unified-timeline.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Upper bound for one playable title `.vob` (part ≥ 1); rejects garbage probe values.
pub(crate) const MAX_VOB_DUR_SEC: f64 = 14_400.0;

/// Drop implausible mpv / probe durations (e.g. DVD9 first `.vob` reporting whole-title length).
#[must_use]
pub(crate) fn clamp_vob_duration(sec: f64) -> f64 {
    if sec.is_finite() && sec > 0.0 && sec <= MAX_VOB_DUR_SEC {
        sec
    } else {
        0.0
    }
}

/// Sorted `.vob` paths for one DVD title (`VTS_XX_*`) and cumulative timing.
pub struct DvdVobTimeline {
    pub vobs: Vec<PathBuf>,
    starts: Vec<f64>,
    durs: Vec<f64>,
    pub total_sec: f64,
}

/// Apply stored duration sources: IFO cell times, menu-stub drop, SQLite map, optional mpv probe.
fn apply_stored_durs(
    tl: &mut DvdVobTimeline,
    current: &Path,
    dur_by_path: &HashMap<String, f64>,
    opts: crate::dvd_entity::TimelineBuildOpts,
) {
    tl.apply_ifo_chapter_durs(current);
    if ifo_timeline_authoritative(current) {
        drop_ifo_stub_segments(tl);
    }
    tl.apply_map_chapter_durs(dur_by_path);
    if opts.mpv_probe && !ifo_timeline_authoritative(current) {
        tl.probe_missing_durs(opts.probe_budget);
    }
}

/// Prefer the live mpv duration when the timeline is not IFO-authoritative.
fn apply_live_chapter_dur_if_usable(
    tl: &mut DvdVobTimeline,
    current: &Path,
    chapter: &Path,
    live_local_dur: f64,
) {
    let live_dur = clamp_vob_duration(live_local_dur);
    let live_chapter = if crate::video_ext::is_dvd_vob_path(current) {
        current
    } else {
        chapter
    };
    if !ifo_timeline_authoritative(current) && live_dur > 0.0 && tl.index_of(live_chapter).is_some()
    {
        tl.apply_live_chapter_dur(live_chapter, live_dur);
    }
}

/// Scrub implausible lengths and infer still-missing ones from siblings.
fn scrub_and_infer_durs(tl: &mut DvdVobTimeline) {
    tl.scrub_implausible_durs();
    if tl.missing_dur_count() > 0 {
        tl.infer_missing_from_siblings();
    }
}

impl DvdVobTimeline {
    /// Build from on-disk title `.vob` files and per-file durations (SQLite / mpv live).
    #[cfg(test)]
    pub fn from_title_vobs(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_local_dur: f64,
    ) -> Option<Self> {
        Self::from_title_vobs_with(
            current,
            dur_by_path,
            live_local_dur,
            crate::dvd_entity::TimelineBuildOpts::PLAYBACK,
        )
    }

    pub(crate) fn from_title_vobs_with(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_local_dur: f64,
        opts: crate::dvd_entity::TimelineBuildOpts,
    ) -> Option<Self> {
        let chapter = crate::dvd_entity::timeline_chapter_probe(current)?;
        let vobs = crate::dvd_entity::timeline_chapter_paths(&chapter)?;
        let n = vobs.len();
        if n == 0 {
            return None;
        }
        let mut tl = Self {
            vobs,
            starts: Vec::new(),
            durs: vec![0.0; n],
            total_sec: 0.0,
        };
        apply_stored_durs(&mut tl, current, dur_by_path, opts);
        apply_live_chapter_dur_if_usable(&mut tl, current, chapter.as_path(), live_local_dur);
        scrub_and_infer_durs(&mut tl);
        tl.recompute_starts();
        (tl.total_sec > 0.0).then_some(tl)
    }

    #[must_use]
    pub(crate) fn missing_dur_count(&self) -> usize {
        self.durs.iter().filter(|d| **d <= 0.0).count()
    }

    /// libmpv headless probe for queued `.vob` files still missing a segment length.
    pub(crate) fn probe_missing_durs(&mut self, budget: Option<usize>) -> usize {
        if self
            .vobs
            .first()
            .is_some_and(|p| ifo_timeline_authoritative(p))
        {
            return self.missing_dur_count();
        }
        let missing = self.missing_dur_count();
        if missing == 0 {
            return 0;
        }
        let cap = budget.unwrap_or(missing);
        let mut left = cap;
        for (i, vob) in self.vobs.iter().enumerate() {
            if self.durs[i] > 0.0 {
                continue;
            }
            if left == 0 {
                break;
            }
            if let Some(d) = crate::dvd_vob_mpv_probe::probe_vob_duration(vob) {
                self.durs[i] = d;
            }
            left -= 1;
        }
        self.recompute_starts();
        self.missing_dur_count()
    }

    /// Overwrite segment lengths from SQLite / mpv per-`.vob` entries.
    pub(crate) fn apply_map_chapter_durs(&mut self, dur_by_path: &HashMap<String, f64>) {
        for (i, vob) in self.vobs.iter().enumerate() {
            if self.durs[i] > 0.0 {
                continue;
            }
            let mapped = dur_from_map(dur_by_path, vob);
            if mapped > 0.0 {
                self.durs[i] = mapped;
            }
        }
        self.recompute_starts();
    }

    /// Prefer mpv `duration` on the open `.vob` when it differs from the stored value.
    #[must_use]
    pub fn global_pos(&self, current: &Path, local_pos: f64) -> f64 {
        let Some(i) = self.index_of(current) else {
            return local_pos.max(0.0);
        };
        (self.starts[i] + local_pos.max(0.0)).min(self.total_sec)
    }

    /// Map unified time to `.vob` index and local offset.
    #[must_use]
    pub fn resolve_global(&self, global: f64) -> (usize, f64) {
        let g = global.clamp(0.0, self.total_sec);
        if self.vobs.is_empty() {
            return (0, 0.0);
        }
        if self.vobs.len() == 1 {
            return (0, g.min(self.durs[0].max(0.0)));
        }
        if let Some(hit) = self.resolve_within_windows(g) {
            return hit;
        }
        self.resolve_after_last_window(g)
    }

    /// Chapter whose `[start, end)` window contains `g`, with the local offset.
    fn resolve_within_windows(&self, g: f64) -> Option<(usize, f64)> {
        for i in 0..self.vobs.len() {
            let d = self.durs[i];
            if d <= 0.0 {
                continue;
            }
            let start = self.starts[i];
            let end = start + d;
            if g >= start && g < end - 1e-3 {
                return Some((i, (g - start).max(0.0)));
            }
        }
        None
    }

    /// Fallback past every window: last chapter with a known length.
    fn resolve_after_last_window(&self, g: f64) -> (usize, f64) {
        let mut last = self.vobs.len() - 1;
        for i in (0..self.vobs.len()).rev() {
            if self.durs[i] > 0.0 {
                last = i;
                break;
            }
        }
        let local = (g - self.starts[last])
            .max(0.0)
            .min(self.durs[last].max(0.0));
        (last, local)
    }

    pub fn path_at(&self, index: usize) -> Option<&Path> {
        self.vobs.get(index).map(|p| p.as_path())
    }

    #[must_use]
    pub fn chapter_dur_at(&self, index: usize) -> f64 {
        self.durs
            .get(index)
            .copied()
            .filter(|d| d.is_finite() && *d > 0.0)
            .unwrap_or(0.0)
    }

    #[must_use]
    pub(crate) fn ifo_segment_local_plausible(&self, chapter: &Path, local_pos: f64) -> bool {
        let Some(idx) = self.index_of(chapter) else {
            return true;
        };
        let seg = self.chapter_dur_at(idx);
        seg <= 0.0 || local_pos <= seg + 1.0
    }

    #[must_use]
    pub(crate) fn clamp_ifo_segment_local(&self, chapter: &Path, local_pos: f64) -> f64 {
        let Some(idx) = self.index_of(chapter) else {
            return local_pos.max(0.0);
        };
        let seg = self.chapter_dur_at(idx);
        if seg <= 0.0 {
            return local_pos.max(0.0);
        }
        local_pos.max(0.0).min(seg)
    }

    /// Next `.vob` file and its whole-title start time (for EOF advance within one title).
    #[must_use]
    pub fn next_chapter_after(&self, current: &Path) -> Option<(PathBuf, f64)> {
        let i = self.index_of(current)?;
        let j = i + 1;
        if j >= self.vobs.len() {
            return None;
        }
        Some((self.vobs[j].clone(), self.starts[j]))
    }

    pub(crate) fn index_of(&self, path: &Path) -> Option<usize> {
        self.vobs
            .iter()
            .position(|p| crate::video_ext::paths_same_file(p, path))
    }

    fn recompute_starts(&mut self) {
        self.starts.resize(self.durs.len(), 0.0);
        let mut total = 0.0_f64;
        for (idx, &d) in self.durs.iter().enumerate() {
            self.starts[idx] = total;
            if d.is_finite() && d > 0.0 {
                total += d;
            }
        }
        self.total_sec = total.max(0.0);
    }
}

include!("dvd_vob_timeline_durs.rs");

/// Cached DVD title timeline for the transport bar (rebuilt on `FileLoaded`, not every tick).
pub struct DvdBarState {
    pub(super) tl: DvdVobTimeline,
    /// IFO PTT chapter marks scaled onto the VOB timeline (seek-bar labels only).
    chapter_labels: Vec<(f64, String)>,
}

include!("dvd_vob_chapter_marks.rs");
include!("dvd_vob_timeline_ifo.rs");
include!("dvd_vob_bar.rs");
include!("dvd_vob_chain_seek.rs");
include!("dvd_vob_chain_bar.rs");
include!("dvd_vob_timeline_transport.rs");
include!("dvd_chapter_eof.rs");

include!("dvd_vob_timeline_resume.rs");

#[cfg(test)]
include!("dvd_vob_timeline_tests.rs");
