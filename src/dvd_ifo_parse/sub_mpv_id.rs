//! Map title-set IFO subtitle slots to mpv `track-list` ids.

use super::streams::{sub_slot_for_src_id, DvdIfoSub};

/// One mpv `track-list` subtitle entry for IFO slot matching.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpvSubTrackMeta {
    pub id: i64,
    pub src_id: Option<i64>,
    pub lang: Option<String>,
}

fn sub_langs_match(ifo_lang: &str, track_lang: &str) -> bool {
    let a = ifo_lang.trim();
    let b = track_lang.trim();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a.eq_ignore_ascii_case(b) || b.starts_with(a) || a.starts_with(b)
}

/// Map a title-set IFO sub slot to an mpv subtitle track id on the open chapter.
#[must_use]
pub fn mpv_sub_id_for_ifo_slot(
    ifo_subs: &[DvdIfoSub],
    tracks: &[MpvSubTrackMeta],
    slot: u8,
) -> Option<i64> {
    for (idx, t) in tracks.iter().enumerate() {
        if sub_slot_for_src_id(ifo_subs, t.src_id, idx) == Some(slot) {
            return Some(t.id);
        }
    }
    if let Some(ifo_row) = ifo_subs.iter().find(|s| s.slot == slot) {
        let want = ifo_row.lang.trim();
        if !want.is_empty() {
            for t in tracks {
                let l = t.lang.as_deref().unwrap_or("").trim();
                if sub_langs_match(want, l) {
                    return Some(t.id);
                }
            }
        }
    }
    if let Some(pos) = ifo_subs.iter().position(|s| s.slot == slot) {
        if let Some(t) = tracks.get(pos) {
            return Some(t.id);
        }
    }
    let dvd_stream = 0x20 + i64::from(slot);
    if tracks.iter().any(|t| t.id == dvd_stream) {
        return Some(dvd_stream);
    }
    None
}

#[cfg(test)]
#[path = "sub_mpv_id_tests.rs"]
mod tests;
