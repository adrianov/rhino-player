//! Virtual DVD timeline: all `.vob` files in one `VIDEO_TS/` as one seek range.
//! See `docs/features/30-dvd-unified-timeline.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Sorted chapter paths for one DVD title (`VTS_XX_*`) and cumulative timing.
pub struct DvdVobTimeline {
    pub vobs: Vec<PathBuf>,
    starts: Vec<f64>,
    durs: Vec<f64>,
    pub total_sec: f64,
    /// PTT chapter marks (global seconds); empty → VOB file boundaries.
    ptt_marks: Vec<f64>,
}

impl DvdVobTimeline {
    /// Build from SQLite / stored chapter lengths only (stable seek-bar range).
    pub fn from_chapter_db_only(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
    ) -> Option<Self> {
        Self::from_chapter_inner(current, dur_by_path, None, 0.0)
    }

    /// Build from any chapter path and per-file durations (seconds). Missing entries count as `0`.
    pub fn from_chapter(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_path: &Path,
        live_local_dur: f64,
    ) -> Option<Self> {
        Self::from_chapter_inner(
            current,
            dur_by_path,
            Some(live_path),
            live_local_dur,
        )
    }

    fn from_chapter_inner(
        current: &Path,
        dur_by_path: &HashMap<String, f64>,
        live_path: Option<&Path>,
        live_local_dur: f64,
    ) -> Option<Self> {
        if !crate::video_ext::is_dvd_vob_path(current) {
            return None;
        }
        let vobs = crate::dvd_entity::list_title_vobs(current.parent()?, current);
        if vobs.is_empty() {
            return None;
        }
        let mut durs: Vec<f64> = vobs
            .iter()
            .map(|p| chapter_duration(p, dur_by_path, live_path, live_local_dur))
            .collect();
        if let Some(i) = live_path.and_then(|live| {
            vobs.iter()
                .position(|p| crate::video_ext::paths_same_file(p, live))
        }) {
            durs[i] = durs[i].max(live_local_dur.max(0.0));
        }
        let entity_key = crate::playback_entity::db_path_for(current);
        let entity_total = entity_key.to_str().and_then(|k| {
            dur_by_path
                .get(k)
                .copied()
                .filter(|d| d.is_finite() && *d > 0.0)
        });
        let mut tl = Self {
            vobs,
            starts: Vec::new(),
            durs,
            total_sec: 0.0,
            ptt_marks: Vec::new(),
        };
        if tl.vobs.len() > 1 {
            if let Some(total) = entity_total {
                tl.apply_entity_total(total);
            } else {
                tl.recompute_starts();
                if tl.durs.iter().any(|d| *d <= 0.0) || tl.total_sec <= 0.0 {
                    tl.bootstrap_from_bytes(current, live_local_dur);
                }
            }
            return Some(tl);
        }
        tl.recompute_starts();
        (tl.total_sec > 0.0).then_some(tl)
    }

    /// Estimate missing chapter lengths from `.vob` file sizes (scaled to `live_dur` on `live_chapter`).
    pub fn bootstrap_from_bytes(&mut self, live_chapter: &Path, live_dur: f64) {
        if self.vobs.len() <= 1 {
            return;
        }
        let bytes: Vec<u64> = self
            .vobs
            .iter()
            .map(|p| p.metadata().ok().map(|m| m.len()).unwrap_or(0))
            .collect();
        let total_b: u64 = bytes.iter().copied().sum();
        if total_b == 0 {
            return;
        }
        let scale = self
            .index_of(live_chapter)
            .filter(|&i| live_dur > 0.0 && bytes[i] > 0)
            .map(|i| live_dur / bytes[i] as f64)
            .unwrap_or(8.0 / 1_000_000.0);
        for (i, b) in bytes.iter().enumerate() {
            let est = (*b as f64) * scale;
            if self.durs[i] <= 0.0 {
                self.durs[i] = est;
            }
        }
        self.recompute_starts();
    }

    /// Apply one stored title duration (equal split when chapter lengths are unknown).
    pub fn apply_entity_total(&mut self, total: f64) {
        if !(total.is_finite() && total > 0.0) {
            return;
        }
        let n = self.vobs.len();
        if n == 0 {
            return;
        }
        let known: f64 = self.durs.iter().filter(|d| **d > 0.0).sum();
        if known <= 0.0 {
            let each = total / n as f64;
            self.durs.fill(each);
        } else if known < total - 0.5 {
            let scale = total / known;
            for d in &mut self.durs {
                if *d > 0.0 {
                    *d *= scale;
                }
            }
        }
        self.recompute_starts();
        if self.total_sec < total - 0.5 {
            if let Some(last) = self.durs.last_mut() {
                *last += total - self.total_sec;
            }
            self.recompute_starts();
        }
    }

    /// Whole-title position for an open chapter and local `time-pos`.
    #[must_use]
    pub fn global_pos(&self, current: &Path, local_pos: f64) -> f64 {
        let Some(i) = self.index_of(current) else {
            return local_pos.max(0.0);
        };
        (self.starts[i] + local_pos.max(0.0)).min(self.total_sec)
    }

    /// Map a whole-title time to chapter index and local offset.
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

    /// Next chapter file and its whole-title start time (for EOF advance within one title).
    #[must_use]
    pub fn next_chapter_after(&self, current: &Path) -> Option<(PathBuf, f64)> {
        let i = self.index_of(current)?;
        let j = i + 1;
        if j >= self.vobs.len() {
            return None;
        }
        Some((self.vobs[j].clone(), self.starts[j]))
    }

    /// Chapter boundary times for seek marks (skip `0.0`).
    pub fn chapter_mark_times(&self) -> Vec<(f64, String)> {
        if !self.ptt_marks.is_empty() {
            return self
                .ptt_marks
                .iter()
                .enumerate()
                .map(|(i, &t)| (t, format!("Chapter {}", i + 2)))
                .collect();
        }
        self.starts
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(i, &t)| {
                self.vobs[i]
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| (t, name.to_string()))
            })
            .collect()
    }

    fn index_of(&self, path: &Path) -> Option<usize> {
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

    /// Extend one chapter when live mpv duration exceeds the stored value.
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

include!("dvd_vob_ifo_build.rs");

/// Cached DVD title timeline for the transport bar (rebuilt on `FileLoaded`, not every tick).
pub struct DvdBarState {
    pub(super) tl: DvdVobTimeline,
}

include!("dvd_vob_bar.rs");
include!("dvd_vob_timeline_transport.rs");
include!("dvd_chapter_eof.rs");

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
