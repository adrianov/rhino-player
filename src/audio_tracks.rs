//! Audio stream list and `aid` for the sound popover. See `docs/features/08-tracks.md`.

use crate::mpv_embed::MpvBundle;
use crate::playback_entity::{
    audio_ifo_slot_for_aid, audio_menu_rows, entity_from_mpv, resolve_audio_mpv_id, AudioMenuRow,
};
use crate::track_label_match::{match_score, LabelMatchScore};
use crate::{db, media_probe, playback_entity};
use libmpv2::Mpv;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;

fn current_aid(mpv: &Mpv) -> Option<i64> {
    if let Ok(s) = mpv.get_property::<String>("aid") {
        if s == "no" {
            return None;
        }
        if let Ok(n) = s.parse::<i64>() {
            return Some(n);
        }
    }
    match mpv.get_property::<i64>("aid") {
        Ok(n) if n > 0 => Some(n),
        _ => None,
    }
}

fn set_aid(mpv: &Mpv, id: i64) {
    if mpv.set_property("aid", id).is_err() {
        eprintln!("[rhino] set aid {id}");
    }
}

fn resolve_id(mpv: &Mpv, row: &AudioMenuRow, shell: Option<&Path>) -> Option<i64> {
    let (entity, _) = entity_from_mpv(mpv, shell)?;
    resolve_audio_mpv_id(mpv, &entity, row, shell)
}

fn restore_audio_by_label(mpv: &Mpv, rows: &[AudioMenuRow], shell: Option<&Path>) {
    if rows.len() < 2 {
        return;
    }
    if let Some(row) = db::load_audio_track_name().and_then(|s| closest_label(rows, &s)) {
        if let Some(aid) = resolve_id(mpv, row, shell) {
            select_aid_if_changed(mpv, aid);
        }
    }
}

fn restore_audio_by_slot(
    mpv: &Mpv,
    entity: &playback_entity::PlaybackEntity,
    slot: u8,
    shell: Option<&Path>,
) -> bool {
    let menu = AudioMenuRow {
        mpv_id: -1,
        label: String::new(),
        ifo_slot: Some(slot),
    };
    let Some(aid) = resolve_audio_mpv_id(mpv, entity, &menu, shell) else {
        return false;
    };
    select_aid_if_changed(mpv, aid);
    true
}

/// Select an audio stream unless it is already active (re-setting reopens the audio path).
fn select_aid_if_changed(mpv: &Mpv, aid: i64) {
    if current_aid(mpv) != Some(aid) {
        set_aid(mpv, aid);
    }
}

/// Restore per-entity track first (IFO slot on DVD, mpv id otherwise), else global label.
pub fn restore_saved_audio(mpv: &Mpv, shell: Option<&Path>) {
    let rows = audio_menu_rows(mpv, shell);
    if rows.is_empty() {
        return;
    }
    let Some(path) = media_probe::shell_media_path(mpv, shell) else {
        return;
    };
    let entity = playback_entity::PlaybackEntity::resolve(&path);
    let Some((saved, saved_slot)) = db::load_audio_track(&entity.db_path()) else {
        restore_audio_by_label(mpv, &rows, shell);
        return;
    };
    if !apply_saved_audio(mpv, &entity, saved, saved_slot, &rows, shell) {
        restore_audio_by_label(mpv, &rows, shell);
    }
}

/// Apply the stored mpv id / DVD slot; returns false when neither matches the track list.
fn apply_saved_audio(
    mpv: &Mpv,
    entity: &playback_entity::PlaybackEntity,
    saved: i64,
    saved_slot: Option<u8>,
    rows: &[AudioMenuRow],
    shell: Option<&Path>,
) -> bool {
    if let Some(slot) = saved_slot {
        if restore_audio_by_slot(mpv, entity, slot, shell) {
            return true;
        }
    }
    if saved > 0 && rows.iter().any(|r| r.mpv_id == saved) {
        select_aid_if_changed(mpv, saved);
        return true;
    }
    false
}

/// Reapply saved audio after cross-chapter DVD `loadfile` once resume seek finishes.
pub fn reapply_after_chapter_load(mpv: &Mpv, shell: Option<&Path>) {
    restore_saved_audio(mpv, shell);
    ensure_playable_audio(mpv, shell);
}

/// After [loadfile], make sure an audio stream is actually selected.
/// With one track, `aid` may be left as `no` until set explicitly; with several, only fixes `aid=no`.
/// Does **not** re-set an already-active id to avoid re-opening the audio path (causes A/V drift).
pub fn ensure_playable_audio(mpv: &Mpv, shell: Option<&Path>) {
    let rows = audio_menu_rows(mpv, shell);
    if rows.is_empty() {
        return;
    }
    if rows.len() == 1 {
        if let Some(want) = resolve_id(mpv, &rows[0], shell) {
            select_aid_if_changed(mpv, want);
        }
        return;
    }
    if matches!(mpv.get_property::<String>("aid"), Ok(s) if s == "no") {
        if let Some(aid) = resolve_id(mpv, &rows[0], shell) {
            set_aid(mpv, aid);
        }
    }
}

/// Label of the currently active audio track, or `None` if no media or no audio.
pub fn current_audio_label(mpv: &Mpv, shell: Option<&Path>) -> Option<String> {
    let rows = audio_menu_rows(mpv, shell);
    if rows.is_empty() {
        return None;
    }
    let want = current_aid(mpv);
    let want_slot = entity_from_mpv(mpv, shell)
        .and_then(|(entity, _)| want.and_then(|a| audio_ifo_slot_for_aid(mpv, &entity, a, shell)));
    rows.iter()
        .find(|r| audio_row_is_active(want, want_slot, r.mpv_id, r.ifo_slot))
        .or_else(|| rows.first())
        .map(|r| r.label.clone())
}

include!("audio_tracks_tooltip.rs");
include!("audio_tracks_popover.rs");

#[cfg(test)]
mod tests {
    use super::audio_row_is_active;

    #[test]
    fn row_active_by_mpv_id_only_when_no_ifo_slots() {
        assert!(audio_row_is_active(Some(2), None, 2, None));
        assert!(!audio_row_is_active(Some(2), None, 1, None));
        // IFO-only rows carry negative mpv ids; they never match by mpv id alone.
        assert!(!audio_row_is_active(Some(-1), None, -1, None));
    }

    #[test]
    fn row_active_by_dvd_slot() {
        assert!(audio_row_is_active(None, Some(1), -1, Some(1)));
        assert!(!audio_row_is_active(None, Some(1), -1, Some(0)));
    }
}
