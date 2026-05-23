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
                kind: PlaybackEntityKind::DvdTitle {
                    db_key,
                    chapters,
                },
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
        let pos = if local_pos.is_finite() { local_pos.max(0.0) } else { 0.0 };
        let dur = if local_dur.is_finite() { local_dur.max(0.0) } else { 0.0 };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn standalone_is_single_file_entity() {
        let base = std::env::temp_dir().join(format!("rhino-pe-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("mkdir");
        let f = base.join("clip.mkv");
        fs::write(&f, b"x").expect("write");
        let ent = PlaybackEntity::resolve(&f);
        assert!(!ent.has_unified_timeline());
        assert_eq!(ent.db_path(), fs::canonicalize(&f).unwrap());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_disc_folder_maps_to_title_entity() {
        let base = std::env::temp_dir().join(format!("rhino-pe-disc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"v").expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), b"v").expect("write");
        let from_vob = PlaybackEntity::resolve(&vts.join("VTS_02_1.VOB"));
        let from_disc = PlaybackEntity::resolve(&base);
        assert!(from_disc.has_unified_timeline());
        let disc_key = std::fs::canonicalize(&base).unwrap_or(base.clone());
        assert_eq!(from_disc.db_path(), disc_key);
        assert_eq!(from_vob.db_path(), from_disc.db_path());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dvd_chapters_share_entity_key() {
        let base = std::env::temp_dir().join(format!("rhino-pe-dvd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        for n in ["VTS_02_1.VOB", "VTS_02_2.VOB"] {
            fs::write(vts.join(n), b"v").expect("write");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let e1 = PlaybackEntity::resolve(&p1);
        let e2 = PlaybackEntity::resolve(&p2);
        assert!(e1.has_unified_timeline());
        assert_eq!(e1.db_path(), e2.db_path());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn card_resume_uses_entity_not_chapter_local() {
        let base = std::env::temp_dir().join(format!("rhino-pe-card-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_0.IFO"), b"IFO").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 2000]).expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let entity = db_path_for(&p1);
        let disc_key = std::fs::canonicalize(&base).unwrap_or(base.clone());
        assert!(crate::video_ext::paths_same_file(&entity, &disc_key));
        let p2k = p2.to_string_lossy().into_owned();
        let mut durs = HashMap::new();
        let mut tpos = HashMap::new();
        durs.insert(p2k.clone(), 100.0);
        tpos.insert(p2k, 50.0);
        let (resume, duration) = card_resume_duration(&p2, &durs, &tpos);
        assert!(duration > 100.0, "expected title duration, got {duration}");
        assert!(resume > 50.0 || resume == 0.0);
        let _ = fs::remove_dir_all(&base);
    }
}
