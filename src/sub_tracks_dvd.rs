// Subtitle rows via `playback_entity` (included by `sub_tracks.rs`).

use std::path::Path;

use crate::playback_entity::{
    entity_from_mpv, resolve_sub_mpv_id, sub_ifo_slot_for_sid, sub_menu_rows, sub_menu_snapshot,
    SubMenuRow,
};

fn row_from_menu(r: &SubMenuRow) -> Row {
    Row {
        id: r.mpv_id,
        text: r.label.clone(),
        lang: r.lang.clone(),
        ifo_slot: r.ifo_slot,
    }
}

fn resolve_sub_id(mpv: &Mpv, id: i64, ifo_slot: Option<u8>, shell: Option<&Path>) -> Option<i64> {
    let (entity, _) = entity_from_mpv(mpv, shell)?;
    resolve_sub_mpv_id(mpv, &entity, id, ifo_slot, shell)
}

fn ifo_slot_for_sid(mpv: &Mpv, sid: i64, shell: Option<&Path>) -> Option<u8> {
    let (entity, _) = entity_from_mpv(mpv, shell)?;
    sub_ifo_slot_for_sid(mpv, &entity, sid, shell)
}

fn sub_rows(mpv: &Mpv, shell: Option<&Path>) -> Vec<Row> {
    sub_menu_rows(mpv, shell).iter().map(row_from_menu).collect()
}

fn sub_popover_data(mpv: &Mpv, shell: Option<&Path>) -> (Vec<Row>, Vec<(i64, String)>) {
    let (menu, codecs) = sub_menu_snapshot(mpv, shell);
    let rows = menu.iter().map(row_from_menu).collect();
    (rows, codecs)
}
