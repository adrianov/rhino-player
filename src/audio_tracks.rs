//! Audio stream list and `aid` for the sound popover. See `docs/features/08-tracks.md`.

use crate::mpv_embed::MpvBundle;
use crate::playback_entity::{
    audio_ifo_slot_for_aid, audio_menu_rows, entity_from_mpv, resolve_audio_mpv_id, AudioMenuRow,
};
use crate::track_label_match::{match_score, LabelMatchScore};
use crate::{db, media_probe, playback_entity};
use libmpv2::Mpv;
use std::cell::{Cell, RefCell};
use std::path::Path;
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

fn save_choice(mpv: &Mpv, id: i64, text: &str, shell: Option<&Path>) {
    db::save_audio_track_name(text);
    let Some(path) = media_probe::shell_media_path(mpv, shell) else {
        return;
    };
    let entity = playback_entity::PlaybackEntity::resolve(&path);
    let slot = playback_entity::audio_ifo_slot_for_aid(mpv, &entity, id, shell);
    db::set_audio_track(&entity.db_path(), id, slot);
}

fn norm_label(s: &str) -> String {
    s.trim().to_lowercase()
}

fn closest_label<'a>(rows: &'a [AudioMenuRow], want: &str) -> Option<&'a AudioMenuRow> {
    let want_n = norm_label(want);
    if want_n.is_empty() {
        return None;
    }
    let mut best_score = LabelMatchScore { word_intersection: 0, char_intersection: 0 };
    let mut picked: Option<&'a AudioMenuRow> = None;
    for row in rows {
        let s = match_score(&want_n, &norm_label(&row.label));
        if picked.is_none() || s > best_score {
            best_score = s;
            picked = Some(row);
        }
    }
    picked
}

fn audio_row_is_active(
    want: Option<i64>,
    want_slot: Option<u8>,
    id: i64,
    ifo_slot: Option<u8>,
) -> bool {
    if want == Some(id) && id > 0 {
        return true;
    }
    matches!((want_slot, ifo_slot), (Some(w), Some(s)) if w == s)
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
            if current_aid(mpv) != Some(aid) {
                set_aid(mpv, aid);
            }
        }
    }
}

fn restore_audio_by_slot(
    mpv: &Mpv,
    entity: &playback_entity::PlaybackEntity,
    slot: u8,
    shell: Option<&Path>,
) -> bool {
    let menu = AudioMenuRow { mpv_id: -1, label: String::new(), ifo_slot: Some(slot) };
    let Some(aid) = resolve_audio_mpv_id(mpv, entity, &menu, shell) else {
        return false;
    };
    if current_aid(mpv) != Some(aid) {
        set_aid(mpv, aid);
    }
    true
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
    if let Some(slot) = saved_slot {
        if restore_audio_by_slot(mpv, &entity, slot, shell) {
            return;
        }
    }
    if saved > 0 && rows.iter().any(|r| r.mpv_id == saved) {
        if current_aid(mpv) != Some(saved) {
            set_aid(mpv, saved);
        }
        return;
    }
    restore_audio_by_label(mpv, &rows, shell);
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
            if current_aid(mpv) != Some(want) {
                set_aid(mpv, want);
            }
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
    let want_slot = entity_from_mpv(mpv, shell).and_then(|(entity, _)| {
        want.and_then(|a| audio_ifo_slot_for_aid(mpv, &entity, a, shell))
    });
    rows.iter()
        .find(|r| audio_row_is_active(want, want_slot, r.mpv_id, r.ifo_slot))
        .or_else(|| rows.first())
        .map(|r| r.label.clone())
}

include!("audio_tracks_tooltip.rs");

/// Rebuilds radio rows. Returns **true** if there are **at least two** audio tracks. Clears the
/// box if there is no player or 0–1 track.
pub fn rebuild_popover(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    bx: &gtk::Box,
    block: &Rc<Cell<bool>>,
    gl: &gtk::GLArea,
    tooltip_btn: Option<&gtk::MenuButton>,
) -> bool {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    let mpv = &b.mpv;
    let shell_path = b.me_budget_shell_path.borrow().clone();
    let shell_ref = shell_path.as_deref();
    let rows = audio_menu_rows(mpv, shell_ref);
    if rows.len() < 2 {
        return false;
    }
    let want = current_aid(mpv);
    let want_slot = entity_from_mpv(mpv, shell_ref).and_then(|(entity, _)| {
        want.and_then(|a| audio_ifo_slot_for_aid(mpv, &entity, a, shell_ref))
    });
    block.set(true);
    let p = Rc::clone(player);
    let blk = Rc::clone(block);
    let gl2 = gl.clone();
    let tip_btn = tooltip_btn.cloned();
    let mut first: Option<gtk::CheckButton> = None;
    let mut buttons: Vec<(i64, Option<u8>, gtk::CheckButton)> = vec![];

    for r in &rows {
        let btn = gtk::CheckButton::with_label(&r.label);
        if let Some(l) = first.as_ref() {
            btn.set_group(Some(l));
        } else {
            first = Some(btn.clone());
        }
        let id = r.mpv_id;
        let ifo_slot = r.ifo_slot;
        let label = r.label.clone();
        let p2 = Rc::clone(&p);
        let blk2 = Rc::clone(&blk);
        let gl3 = gl2.clone();
        let shell_pick = shell_path.clone();
        let tip_btn_pick = tip_btn.clone();
        btn.connect_toggled(move |b| {
            if blk2.get() || !b.is_active() {
                return;
            }
            if let Some(pl) = p2.borrow().as_ref() {
                let shell_ref = shell_pick.as_deref();
                let row = AudioMenuRow { mpv_id: id, label: label.clone(), ifo_slot };
                if let Some(aid) = resolve_id(&pl.mpv, &row, shell_ref) {
                    set_aid(&pl.mpv, aid);
                    save_choice(&pl.mpv, aid, &label, shell_ref);
                }
                if let Some(ref tip_btn) = tip_btn_pick {
                    refresh_audio_tooltip(&pl.mpv, tip_btn, shell_ref);
                }
            }
            gl3.queue_render();
        });
        buttons.push((id, ifo_slot, btn));
    }
    for (_, _, btn) in &buttons {
        bx.append(btn);
    }
    for (id, ifo_slot, btn) in &buttons {
        btn.set_active(audio_row_is_active(want, want_slot, *id, *ifo_slot));
    }
    block.set(false);
    true
}

#[cfg(test)]
mod tests {
    use super::audio_row_is_active;

    #[test]
    fn row_active_by_mpv_id_only_when_no_ifo_slots() {
        assert!(audio_row_is_active(Some(2), None, 2, None));
        assert!(!audio_row_is_active(Some(2), None, 1, None));
        assert!(!audio_row_is_active(None, None, 1, None));
    }

    #[test]
    fn row_active_by_dvd_slot() {
        assert!(audio_row_is_active(None, Some(1), -1, Some(1)));
        assert!(!audio_row_is_active(None, Some(1), -1, Some(0)));
    }
}
