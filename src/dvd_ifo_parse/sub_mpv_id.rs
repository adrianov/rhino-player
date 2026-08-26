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
    track_id_by_src_slot(ifo_subs, tracks, slot)
        .or_else(|| track_id_by_ifo_lang(ifo_subs, tracks, slot))
        .or_else(|| track_id_by_position(ifo_subs, tracks, slot))
        .or_else(|| dvd_stream_fallback(tracks, slot))
}

fn track_id_by_src_slot(
    ifo_subs: &[DvdIfoSub],
    tracks: &[MpvSubTrackMeta],
    slot: u8,
) -> Option<i64> {
    tracks
        .iter()
        .enumerate()
        .find(|(idx, t)| sub_slot_for_src_id(ifo_subs, t.src_id, *idx) == Some(slot))
        .map(|(_, t)| t.id)
}

fn track_id_by_ifo_lang(
    ifo_subs: &[DvdIfoSub],
    tracks: &[MpvSubTrackMeta],
    slot: u8,
) -> Option<i64> {
    let want = ifo_subs.iter().find(|s| s.slot == slot)?.lang.trim();
    if want.is_empty() {
        return None;
    }
    tracks
        .iter()
        .find(|t| sub_langs_match(want, t.lang.as_deref().unwrap_or("").trim()))
        .map(|t| t.id)
}

fn track_id_by_position(
    ifo_subs: &[DvdIfoSub],
    tracks: &[MpvSubTrackMeta],
    slot: u8,
) -> Option<i64> {
    let pos = ifo_subs.iter().position(|s| s.slot == slot)?;
    tracks.get(pos).map(|t| t.id)
}

fn dvd_stream_fallback(tracks: &[MpvSubTrackMeta], slot: u8) -> Option<i64> {
    let dvd_stream = 0x20 + i64::from(slot);
    tracks
        .iter()
        .any(|t| t.id == dvd_stream)
        .then_some(dvd_stream)
}

#[cfg(test)]
#[path = "sub_mpv_id_tests.rs"]
mod tests;
