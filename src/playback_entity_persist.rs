// Persist resume / duration through the playback entity row only.

use std::collections::HashMap;
use std::path::Path;

use libmpv2::Mpv;

use super::PlaybackEntity;
use crate::media_probe::{self, NEAR_END_SEC};

/// Drop stored resume for the entity keyed by any chapter or folder path.
pub fn clear_entity_resume(path: &Path) {
    let ent = PlaybackEntity::resolve(path);
    crate::db::clear_resume_position(&ent.db_path());
    ent.purge_extra_db_rows();
}

/// Whole-title `(duration_sec, time_pos_sec)` → SQLite entity row; purges per-chapter aliases.
pub fn persist_playback(
    playing: &Path,
    local_pos: f64,
    local_dur: f64,
    dur_by_path: &HashMap<String, f64>,
) {
    let ent = PlaybackEntity::resolve(playing);
    let (total, global) = ent.playback_snapshot(playing, local_pos, local_dur, dur_by_path);
    if !total.is_finite() || total <= 0.0 {
        return;
    }
    crate::db::set_playback(&ent.db_path(), total, global);
    if ent.has_unified_timeline()
        && local_dur.is_finite()
        && local_dur > 0.0
        && crate::video_ext::is_dvd_vob_path(playing)
    {
        crate::db::set_duration(playing, local_dur);
    }
}

/// Snapshot mpv transport into the entity row (unified timeline for multi-part DVDs).
pub fn persist_from_mpv(mpv: &Mpv, shell: Option<&Path>) {
    let Some(playing) = media_probe::shell_media_path(mpv, shell) else {
        return;
    };
    let ent = PlaybackEntity::resolve(&playing);
    let at_tail = media_probe::is_natural_end(mpv);
    if at_tail && !ent.has_unified_timeline() {
        clear_entity_resume(&playing);
        return;
    }
    let map = crate::db::load_duration_map();
    let dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0);
    let pos = mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0);
    match (dur, pos) {
        (Some(dur), Some(pos)) => persist_playback(&playing, pos, dur, &map),
        (Some(dur), None) => {
            let (total, _) = ent.playback_snapshot(&playing, 0.0, dur, &map);
            if total.is_finite() && total > 0.0 {
                crate::db::set_duration(&ent.db_path(), total);
                ent.purge_extra_db_rows();
            }
        }
        _ => {}
    }
    if at_tail && ent.has_unified_timeline() {
        if let (Some(dur), Some(pos)) = (dur, pos) {
            let (total, global) = ent.playback_snapshot(&playing, pos, dur, &map);
            if total > 5.0 && global >= total - NEAR_END_SEC {
                clear_entity_resume(&playing);
            }
        }
    }
}
