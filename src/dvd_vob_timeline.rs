//! Virtual DVD timeline: queued `.vob` files for one title set and cumulative timing.
//! See `docs/features/30-dvd-unified-timeline.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Upper bound for one playable title `.vob` (part ≥ 1); rejects byte-bootstrap garbage.
const MAX_VOB_DUR_SEC: f64 = 14_400.0;

/// Sorted `.vob` paths for one DVD title (`VTS_XX_*`) and cumulative timing.
pub struct DvdVobTimeline {
    pub vobs: Vec<PathBuf>,
    starts: Vec<f64>,
    durs: Vec<f64>,
    pub total_sec: f64,
}

impl DvdVobTimeline {
    /// Build from on-disk title `.vob` files and per-file durations (SQLite / mpv live).
    pub fn from_title_vobs(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_path: Option<&Path>,
        live_local_dur: f64,
    ) -> Option<Self> {
        Self::from_title_vobs_inner(current, dur_by_path, live_path, live_local_dur)
    }

    fn from_title_vobs_inner(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_path: Option<&Path>,
        live_local_dur: f64,
    ) -> Option<Self> {
        if !crate::video_ext::is_dvd_vob_path(current) {
            return None;
        }
        let vobs = crate::dvd_entity::list_title_vobs(current.parent()?, current);
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
        tl.apply_map_chapter_durs(dur_by_path);
        if let Some(live) = live_path {
            if live_local_dur > 0.0 {
                tl.apply_live_chapter_dur(live, live_local_dur);
            }
        }
        if tl.durs.iter().any(|d| *d <= 0.0) {
            tl.fill_missing_durs_from_bytes(live_path.unwrap_or(current), live_local_dur);
        }
        tl.recompute_starts();
        (tl.total_sec > 0.0).then_some(tl)
    }

    /// Overwrite segment lengths from SQLite / mpv per-`.vob` entries.
    pub(crate) fn apply_map_chapter_durs(&mut self, dur_by_path: &HashMap<String, f64>) {
        for (i, vob) in self.vobs.iter().enumerate() {
            let mapped = dur_from_map(dur_by_path, vob);
            if mapped > 0.0 {
                self.durs[i] = mapped;
            }
        }
        self.recompute_starts();
    }

    /// Fill unknown segment lengths from `.vob` sizes; requires mpv live time or another known segment.
    pub fn fill_missing_durs_from_bytes(&mut self, anchor_vob: &Path, anchor_dur: f64) {
        if self.vobs.len() <= 1 {
            return;
        }
        let bytes: Vec<u64> = self
            .vobs
            .iter()
            .map(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
            .collect();
        if bytes.iter().all(|&b| b == 0) {
            return;
        }
        let scale = if anchor_dur > 0.0 {
            self.index_of(anchor_vob)
                .filter(|&i| bytes[i] > 0)
                .map(|i| anchor_dur / bytes[i] as f64)
        } else {
            None
        };
        let byte_rate = scale.or_else(|| {
            let mut sum_d = 0.0_f64;
            let mut sum_b = 0_u64;
            for (i, &d) in self.durs.iter().enumerate() {
                if d > 0.0 && bytes[i] > 0 {
                    sum_d += d;
                    sum_b += bytes[i];
                }
            }
            (sum_b > 0 && sum_d > 0.0).then_some(sum_d / sum_b as f64)
        });
        let Some(rate) = byte_rate.filter(|r| r.is_finite() && *r > 0.0) else {
            return;
        };
        for (i, b) in bytes.iter().enumerate() {
            if self.durs[i] <= 0.0 {
                let est = (*b as f64) * rate;
                if plausible_vob_dur(est) {
                    self.durs[i] = est;
                }
            }
        }
        self.recompute_starts();
    }

    /// Prefer mpv `duration` on the open `.vob` when it differs from the stored value.
    pub(crate) fn apply_live_chapter_dur(&mut self, chapter: &Path, live_dur: f64) {
        if !(live_dur.is_finite() && live_dur > 0.0) {
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
        for i in 0..self.vobs.len() {
            let d = self.durs[i];
            if d <= 0.0 {
                continue;
            }
            let start = self.starts[i];
            let end = start + d;
            if g >= start && g < end - 1e-3 {
                return (i, (g - start).max(0.0));
            }
        }
        let mut last = self.vobs.len() - 1;
        for i in (0..self.vobs.len()).rev() {
            if self.durs[i] > 0.0 {
                last = i;
                break;
            }
        }
        let local = (g - self.starts[last]).max(0.0).min(self.durs[last].max(0.0));
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

    /// Extend one segment when live mpv duration exceeds the stored value.
    pub fn grow_chapter_dur(&mut self, chapter: &Path, live_dur: f64) {
        if !(live_dur.is_finite() && live_dur > 0.0) {
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
}

/// Cached DVD title timeline for the transport bar (rebuilt on `FileLoaded`, not every tick).
pub struct DvdBarState {
    pub(super) tl: DvdVobTimeline,
    /// IFO PTT chapter marks scaled onto the VOB timeline (seek-bar labels only).
    chapter_labels: Vec<(f64, String)>,
}

include!("dvd_vob_chapter_marks.rs");
include!("dvd_vob_bar.rs");
include!("dvd_vob_timeline_transport.rs");
include!("dvd_chapter_eof.rs");

fn plausible_vob_dur(d: f64) -> bool {
    d.is_finite() && d > 0.0 && d <= MAX_VOB_DUR_SEC
}

fn chapter_duration(
    path: &Path,
    dur_by_path: &HashMap<String, f64>,
    live_path: Option<&Path>,
    live_local_dur: f64,
) -> f64 {
    if live_path.is_some_and(|live| crate::video_ext::paths_same_file(path, live)) {
        return live_local_dur.max(0.0);
    }
    dur_from_map(dur_by_path, path)
}

#[cfg(test)]
include!("dvd_vob_timeline_tests.rs");
