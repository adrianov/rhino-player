//! Matching of mpv `track-list` rows onto DVD IFO stream slots and labels.

use super::{DvdIfoAudio, DvdIfoSub};

/// mpv `track-list` fields used to merge DVD IFO labels when `lang` / `title` are absent.
#[derive(Clone, Copy, Debug, Default)]
pub struct MpvTrackMeta<'a> {
    pub src_id: Option<i64>,
    pub codec: Option<&'a str>,
    pub demux_channels: Option<i64>,
}

/// Map one mpv audio row to its VTS IFO stream slot (no `used` bookkeeping).
pub fn audio_slot_for_meta(streams: &[DvdIfoAudio], meta: MpvTrackMeta<'_>) -> Option<u8> {
    if streams.is_empty() {
        return None;
    }
    let mpv_codec = meta.codec?;
    let mpv_ch = meta.demux_channels.unwrap_or(0).max(0) as u8;
    let candidates: Vec<usize> = streams
        .iter()
        .enumerate()
        .filter(|(_, s)| s.codec_key == mpv_codec && channels_match(s.channels, mpv_ch))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let pick = if candidates.len() == 1 {
        candidates[0]
    } else {
        pick_by_src_id(streams, &candidates, meta.src_id)?
    };
    Some(streams[pick].slot)
}

/// Pick an IFO audio label for one mpv audio row; `used` holds already-matched IFO slots.
pub fn match_audio_label(
    streams: &[DvdIfoAudio],
    meta: MpvTrackMeta<'_>,
    used: &mut [bool],
) -> Option<String> {
    let slot = audio_slot_for_meta(streams, meta)?;
    let pick = streams.iter().position(|s| s.slot == slot)?;
    if used.get(pick).copied().unwrap_or(false) {
        return None;
    }
    if let Some(u) = used.get_mut(pick) {
        *u = true;
    }
    Some(streams[pick].label.clone())
}

fn channels_match(ifo_ch: u8, mpv_ch: u8) -> bool {
    if ifo_ch == mpv_ch {
        return true;
    }
    matches!((ifo_ch, mpv_ch), (5, 6) | (6, 5))
}

fn pick_by_src_id(
    streams: &[DvdIfoAudio],
    candidates: &[usize],
    src_id: Option<i64>,
) -> Option<usize> {
    let sid = src_id? as u8;
    for &i in candidates {
        let slot = streams[i].slot;
        if src_id_matches_slot(sid, slot, streams[i].codec_key) {
            return Some(i);
        }
    }
    candidates.first().copied()
}

fn src_id_matches_slot(sid: u8, slot: u8, codec: &str) -> bool {
    if (0x88..=0x8f).contains(&sid) && codec == "dts" {
        return sid.saturating_sub(0x88) == slot;
    }
    if (0x80..=0x87).contains(&sid) {
        return sid.saturating_sub(0x80) == slot;
    }
    if (0xa0..=0xa7).contains(&sid) && codec == "lpcm" {
        return sid.saturating_sub(0xa0) == slot;
    }
    false
}

pub fn match_sub_label(streams: &[DvdIfoSub], slot: usize, used: &mut [bool]) -> Option<String> {
    let row = streams.get(slot)?;
    if used.get(slot).copied().unwrap_or(false) {
        return None;
    }
    if let Some(u) = used.get_mut(slot) {
        *u = true;
    }
    Some(row.label.clone())
}

/// DVD sub stream slot from mpv `demuxer-src-id` (0x20–0x3f) or list order.
pub fn sub_slot_for_src_id(
    streams: &[DvdIfoSub],
    src_id: Option<i64>,
    fallback_idx: usize,
) -> Option<u8> {
    if let Some(sid) = src_id {
        let slot = sid as u8;
        if (0x20..=0x3f).contains(&slot) {
            let s = slot - 0x20;
            if streams.iter().any(|r| r.slot == s) {
                return Some(s);
            }
        }
    }
    streams.get(fallback_idx).map(|r| r.slot)
}
