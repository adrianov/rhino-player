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
    if ent.has_unified_timeline() {
        ent.save_global_resume(total, global);
    } else {
        crate::db::set_playback(&ent.db_path(), total, global);
        ent.purge_extra_db_rows();
        crate::media_probe::continue_grid_cache_note_playback(&ent.db_path(), global, total);
    }
}

/// mpv `(duration, time-pos)` snapshot, each filtered to a sane value.
fn mpv_dur_pos(mpv: &Mpv) -> (Option<f64>, Option<f64>) {
    let dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0);
    let pos = mpv
        .get_property::<f64>("time-pos")
        .ok()
        .filter(|p| p.is_finite() && *p >= 0.0);
    (dur, pos)
}

/// Unified-timeline transport-bar seconds → entity row (no-op without a usable bar).
fn save_unified_bar(ent: &PlaybackEntity, transport_bar: Option<(f64, f64)>) {
    if let Some((total, global)) = transport_bar.filter(|_| ent.has_unified_timeline()) {
        if total.is_finite() && total > 0.0 && global.is_finite() {
            ent.save_global_resume(total, global);
        }
    }
}

/// Plain single-file persistence: full playback snapshot, or duration-only without a position.
fn persist_plain(mpv: &Mpv, ent: &PlaybackEntity, playing: &Path, map: &HashMap<String, f64>) {
    if ent.has_unified_timeline() {
        return;
    }
    let (dur, pos) = mpv_dur_pos(mpv);
    match (dur, pos) {
        (Some(dur), Some(pos)) => persist_playback(playing, pos, dur, map),
        (Some(dur), None) => {
            let (total, _) = ent.playback_snapshot(playing, 0.0, dur, map);
            if total.is_finite() && total > 0.0 {
                crate::db::set_duration(&ent.db_path(), total);
                ent.purge_extra_db_rows();
            }
        }
        _ => {}
    }
}

/// Natural end on a unified timeline drops the stored resume once playback is near the title tail.
fn clear_unified_at_tail(
    mpv: &Mpv,
    ent: &PlaybackEntity,
    playing: &Path,
    transport_bar: Option<(f64, f64)>,
    map: &HashMap<String, f64>,
    at_tail: bool,
) {
    if !ent.has_unified_timeline() || !at_tail {
        return;
    }
    let (dur, pos) = mpv_dur_pos(mpv);
    if let (Some(dur), Some(pos)) = (dur, pos) {
        let (total, global) =
            transport_bar.unwrap_or_else(|| ent.playback_snapshot(playing, pos, dur, map));
        if total > 5.0 && global >= total - NEAR_END_SEC {
            clear_entity_resume(playing);
        }
    }
}

/// Snapshot mpv transport into the entity row (unified timeline for multi-part DVDs).
pub fn persist_from_mpv(mpv: &Mpv, shell: Option<&Path>, transport_bar: Option<(f64, f64)>) {
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
    save_unified_bar(&ent, transport_bar);
    persist_plain(mpv, &ent, &playing, &map);
    clear_unified_at_tail(mpv, &ent, &playing, transport_bar, &map, at_tail);
}
