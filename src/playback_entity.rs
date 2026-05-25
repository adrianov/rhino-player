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
}
pub use transport::{
    open_playback, preview_hover_duration_for_open, preview_seek_plan_for_open, transport_chapter_path,
    unified_timeline_chapter,
};

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
        let p1k = p1.to_string_lossy().into_owned();
        let p2k = p2.to_string_lossy().into_owned();
        let mut durs = HashMap::new();
        let mut tpos = HashMap::new();
        durs.insert(p1k, 100.0);
        durs.insert(p2k.clone(), 100.0);
        tpos.insert(p2k, 50.0);
        let (resume, duration) = card_resume_duration(&p2, &durs, &tpos);
        assert!(duration > 100.0, "expected title duration, got {duration}");
        assert!(resume > 50.0 || resume == 0.0);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resume_load_target_maps_global_to_chapter_vob() {
        let base = std::env::temp_dir().join(format!("rhino-pe-thumb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), b"a").expect("vob1");
        fs::write(vts.join("VTS_02_2.VOB"), b"b").expect("vob2");
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let entity = PlaybackEntity::resolve(&p1);
        let mut durs = HashMap::new();
        durs.insert(entity.db_path().to_string_lossy().into_owned(), 150.0);
        durs.insert(p1.to_string_lossy().into_owned(), 100.0);
        durs.insert(p2.to_string_lossy().into_owned(), 50.0);
        let (load, local) = entity
            .resume_load_target(&p1, 120.0, &durs)
            .expect("chapter target");
        assert!(crate::video_ext::paths_same_file(&load, &p2));
        assert!((local - 20.0).abs() < 1e-3);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn card_resume_keeps_global_past_stale_entity_duration() {
        let base = std::env::temp_dir().join(format!("rhino-pe-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        for (i, n) in [(1, 100usize), (2, 200), (3, 300), (4, 400)] {
            fs::write(vts.join(format!("VTS_02_{i}.VOB")), vec![b'x'; n]).expect("vob");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let entity = db_path_for(&p1);
        let ek = entity.to_string_lossy().into_owned();
        let mut durs = HashMap::new();
        let mut tpos = HashMap::new();
        durs.insert(ek.clone(), 100.0);
        tpos.insert(ek, 130.0);
        durs.insert(p1.to_string_lossy().into_owned(), 100.0);
        let (resume, duration) = card_resume_duration(&entity, &durs, &tpos);
        assert!(resume > 100.0, "resume should stay global, got {resume}");
        assert!(duration >= resume, "duration {duration} should cover resume {resume}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn title_set_streams_match_on_every_chapter() {
        let vob = Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD2/Video_ts/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let p2 = vob.with_file_name("VTS_02_2.VOB");
        if !p2.is_file() {
            return;
        }
        let e1 = PlaybackEntity::resolve(vob);
        let e2 = PlaybackEntity::resolve(&p2);
        let s1 = e1.title_set_streams(vob);
        let s2 = e2.title_set_streams(&p2);
        assert!(s1.is_some());
        assert_eq!(s1, s2);
    }
}
