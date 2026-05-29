// Persist / restore subtitle choice per playback entity (included from `sub_tracks.rs`).

use crate::playback_entity;

/// Store the resolved mpv `sid` and optional DVD IFO slot on the playback-entity key.
pub fn save_sub_choice(mpv: &Mpv, sid: i64, ifo_slot: Option<u8>, shell: Option<&std::path::Path>) {
    if sid <= 0 {
        return;
    }
    let Some(path) = crate::media_probe::shell_media_path(mpv, shell) else {
        return;
    };
    let entity = playback_entity::PlaybackEntity::resolve(&path);
    crate::db::set_sub_track(&entity.db_path(), sid, ifo_slot);
}

/// Reapply the saved subtitle for this entity (IFO slot on DVD, mpv id otherwise).
#[must_use]
pub fn restore_saved_sub(mpv: &Mpv, prefs: &SubPrefs, shell: Option<&std::path::Path>) -> bool {
    if prefs.sub_off {
        set_sub_off(mpv);
        return true;
    }
    let rows = sub_rows(mpv, shell);
    if rows.is_empty() {
        return false;
    }
    let Some(path) = crate::media_probe::shell_media_path(mpv, shell) else {
        return false;
    };
    let entity = playback_entity::PlaybackEntity::resolve(&path);
    let Some((saved_sid, saved_slot)) = crate::db::load_sub_track(&entity.db_path()) else {
        return false;
    };
    if let Some(slot) = saved_slot {
        if let Some(sid) = resolve_sub_id(mpv, saved_sid, Some(slot), shell) {
            if current_sid(mpv) != Some(sid) {
                set_sub_id(mpv, sid);
            }
            reapply_styling(mpv);
            return true;
        }
    }
    if saved_sid > 0 && rows.iter().any(|r| r.id == saved_sid) {
        if current_sid(mpv) != Some(saved_sid) {
            set_sub_id(mpv, saved_sid);
        }
        reapply_styling(mpv);
        return true;
    }
    false
}
