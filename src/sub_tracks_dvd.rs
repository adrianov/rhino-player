// Subtitle rows via `playback_entity` (included by `sub_tracks.rs`).

use crate::playback_entity::{
    entity_from_mpv, resolve_sub_mpv_id, sub_ifo_slot_for_sid, sub_menu_rows, SubMenuRow,
};

fn row_from_menu(r: &SubMenuRow) -> Row {
    Row {
        id: r.mpv_id,
        text: r.label.clone(),
        lang: r.lang.clone(),
        ifo_slot: r.ifo_slot,
    }
}

fn resolve_sub_id(mpv: &Mpv, id: i64, ifo_slot: Option<u8>) -> Option<i64> {
    let (entity, _) = entity_from_mpv(mpv)?;
    resolve_sub_mpv_id(mpv, &entity, id, ifo_slot)
}

fn ifo_slot_for_sid(mpv: &Mpv, sid: i64) -> Option<u8> {
    let (entity, _) = entity_from_mpv(mpv)?;
    sub_ifo_slot_for_sid(mpv, &entity, sid)
}

fn sub_rows(mpv: &Mpv) -> Vec<Row> {
    sub_menu_rows(mpv).iter().map(row_from_menu).collect()
}
