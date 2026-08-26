//! **Playback entity** — one logical title in history / resume / unified transport.
//!
//! Standalone files map 1:1; DVD chapter `.vob` files in the same title set share one row.
//! Call sites use this module instead of branching on `video_ext::is_dvd_vob_path`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How on-disk files group for persistence and transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackEntityKind {
    /// One path is the whole title (mkv, mp4, Blu-ray folder, single `.vob`, …).
    SingleFile(PathBuf),
    /// Several chapter `.vob` files share one timeline and SQLite row.
    DvdTitle {
        db_key: PathBuf,
        chapters: Vec<PathBuf>,
    },
}

/// Resolved grouping for a path the user opened or is playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackEntity {
    kind: PlaybackEntityKind,
}

impl PlaybackEntity {
    /// Classify any openable media path (file, DVD/Blu-ray folder, or chapter `.vob`).
    #[must_use]
    pub fn resolve(path: &Path) -> Self {
        if let Some((db_key, chapters)) = crate::dvd_entity::title_playback_entity(path) {
            return Self {
                kind: PlaybackEntityKind::DvdTitle { db_key, chapters },
            };
        }
        let file = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        Self {
            kind: PlaybackEntityKind::SingleFile(file),
        }
    }

    /// SQLite / history key (canonical when possible).
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        match &self.kind {
            PlaybackEntityKind::SingleFile(p) => p.clone(),
            PlaybackEntityKind::DvdTitle { db_key, .. } => db_key.clone(),
        }
    }

    /// Unified seek bar + global resume (DVD title sets only).
    #[must_use]
    pub(crate) fn has_unified_timeline(&self) -> bool {
        matches!(self.kind, PlaybackEntityKind::DvdTitle { .. })
    }

    /// Map stored resume seconds → `(loadfile path, local offset)`.
    #[must_use]
    pub fn resume_load_target(
        &self,
        opened: &Path,
        stored_sec: f64,
        dur_by_path: &HashMap<String, f64>,
    ) -> Option<(PathBuf, f64)> {
        match &self.kind {
            PlaybackEntityKind::SingleFile(_) => {
                let canon = std::fs::canonicalize(opened).unwrap_or_else(|_| opened.to_path_buf());
                Some((canon, stored_sec))
            }
            PlaybackEntityKind::DvdTitle { .. } => {
                crate::dvd_entity::resume_chapter_and_local(opened, stored_sec, dur_by_path)
            }
        }
    }

    /// Whole-title `(duration_sec, time_pos_sec)` for the persistent store.
    #[must_use]
    pub fn playback_snapshot(
        &self,
        playing: &Path,
        local_pos: f64,
        local_dur: f64,
        dur_by_path: &HashMap<String, f64>,
    ) -> (f64, f64) {
        if let PlaybackEntityKind::DvdTitle { .. } = &self.kind {
            if let Some((total, global)) =
                crate::dvd_entity::playback_snapshot(playing, local_pos, local_dur, dur_by_path)
            {
                return (total, global);
            }
            return (0.0, 0.0);
        }
        let pos = if local_pos.is_finite() {
            local_pos.max(0.0)
        } else {
            0.0
        };
        let dur = if local_dur.is_finite() {
            local_dur.max(0.0)
        } else {
            0.0
        };
        (dur, pos)
    }

    /// Drop stale per-chapter rows after writing the entity row (DVD only).
    pub fn purge_extra_db_rows(&self) {
        if let PlaybackEntityKind::DvdTitle { db_key, .. } = &self.kind {
            crate::dvd_entity::purge_chapter_media_rows(db_key);
        }
    }
}

/// History / `media` path key for any openable path.
#[must_use]
pub fn db_path_for(path: &Path) -> PathBuf {
    PlaybackEntity::resolve(path).db_path()
}

/// Convenience: purge extra rows after a write keyed by any chapter/path.
pub fn purge_extra_db_rows(path: &Path) {
    PlaybackEntity::resolve(path).purge_extra_db_rows();
}

mod persist {
    include!("playback_entity_persist.rs");
}
pub use persist::{clear_entity_resume, persist_from_mpv};

mod dvd_card {
    include!("playback_entity_dvd_card.rs");
}
pub use dvd_card::card_resume_duration;

mod tracks {
    include!("playback_entity_tracks.rs");
}
pub use tracks::{
    audio_ifo_slot_for_aid, audio_menu_rows, entity_from_mpv, entity_has_subtitles,
    resolve_audio_mpv_id, resolve_sub_mpv_id, sub_ifo_slot_for_sid, sub_menu_rows,
    sub_menu_snapshot, AudioMenuRow, SubMenuRow,
};

mod title {
    include!("playback_entity_title.rs");
}
pub use title::window_title_for;

mod transport {
    include!("playback_entity_transport.rs");
    include!("playback_entity_transport_preview.rs");

    #[cfg(test)]
    mod transport_tests {
        include!("playback_entity_transport_tests.rs");
    }
}
pub use transport::{
    open_playback, preview_hover_duration_for_open, preview_seek_plan_for_open,
    transport_chapter_path, unified_timeline_chapter,
};

#[cfg(test)]
mod tests {
    include!("playback_entity_tests.rs");
}
