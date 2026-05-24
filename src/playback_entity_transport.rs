// Unified transport bar clock: single files vs multi-chapter DVD title entities.

use std::path::{Path, PathBuf};

use libmpv2::Mpv;

use super::PlaybackEntity;
use crate::dvd_vob_timeline::DvdBarState;

/// Local chapter path for the seek bar (playing file or warm-preload hover target).
#[must_use]
pub fn transport_chapter_path(
    recent_browse: bool,
    warm_last_path: Option<PathBuf>,
    mpv: Option<&Mpv>,
    shell: Option<&Path>,
) -> Option<PathBuf> {
    if recent_browse {
        return warm_last_path;
    }
    mpv.and_then(|m| crate::media_probe::shell_media_path(m, shell))
}

impl PlaybackEntity {
    /// Cached DVD bar applies only to a **title entity** and an on-timeline chapter.
    #[must_use]
    pub fn dvd_bar_active(&self, chapter: &Path, bar: &DvdBarState) -> bool {
        self.has_unified_timeline() && bar.tl.index_of(chapter).is_some()
    }

    /// Title-wide duration from a cached bar (None for single-file entities or stale bar).
    #[must_use]
    pub fn transport_duration_from_bar(
        &self,
        chapter: &Path,
        bar: &DvdBarState,
    ) -> Option<f64> {
        self.dvd_bar_active(chapter, bar)
            .then_some(bar.total_sec())
    }

    /// Seek-bar `(duration, position)` for this entity (unified timeline when a bar matches).
    #[must_use]
    pub fn transport_bar(
        &self,
        chapter: &Path,
        local_pos: f64,
        local_dur: f64,
        bar: Option<&DvdBarState>,
        bundle: Option<&crate::mpv_embed::MpvBundle>,
    ) -> (f64, f64) {
        let local_pos = if local_pos.is_finite() {
            local_pos.max(0.0)
        } else {
            0.0
        };
        let local_dur = if local_dur.is_finite() {
            local_dur.max(0.0)
        } else {
            0.0
        };
        if let Some(bar) = bar.filter(|b| self.dvd_bar_active(chapter, b)) {
            let dur = bar.total_sec();
            let pos = bundle
                .map(|b| bar.transport_global_pos(b, chapter, local_pos))
                .unwrap_or_else(|| bar.global_pos(chapter, local_pos));
            return (dur, pos.max(0.0));
        }
        (local_dur, local_pos)
    }
}

#[cfg(test)]
mod transport_tests {
    include!("playback_entity_transport_tests.rs");
}

/// Open chapter path when mpv/shell resolves to a **title entity** (multi-chapter DVD timeline).
#[must_use]
pub fn unified_timeline_chapter(
    mpv: &Mpv,
    shell: Option<&Path>,
) -> Option<PathBuf> {
    let path = crate::media_probe::local_file_from_mpv(mpv).or_else(|| {
        shell.and_then(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.to_path_buf())))
    })?;
    PlaybackEntity::resolve(&path)
        .has_unified_timeline()
        .then_some(path)
}
