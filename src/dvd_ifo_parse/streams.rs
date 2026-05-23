//! VTS IFO audio / subpicture stream attributes (`vtsi_mat_t`), aligned with libdvdread layout.

use std::path::Path;

use super::bitreader::{read_audio_attr, read_subp_attr};
use super::buf::IfoBuf;
use super::vts_id_from_path;

// `offsetof(vtsi_mat_t, …)` from libdvdread 7.x (Homebrew); stable for VTS_xx_0.IFO.
const NR_AUDIO_OFF: usize = 515;
const AUDIO_OFF: usize = 516;
const NR_SUB_OFF: usize = 597;
const SUB_OFF: usize = 598;
const AUDIO_ATTR_SIZE: usize = 8;
const SUBP_ATTR_SIZE: usize = 6;
const MAX_AUDIO: usize = 8;
const MAX_SUB: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DvdIfoAudio {
    pub slot: u8,
    pub lang: String,
    pub channels: u8,
    pub label: String,
    pub(super) codec_key: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DvdIfoSub {
    pub slot: u8,
    pub lang: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DvdIfoStreams {
    pub audio: Vec<DvdIfoAudio>,
    pub sub: Vec<DvdIfoSub>,
}

/// Parsed stream metadata for the title set of a chapter `.vob`.
pub fn streams_from_vob(vob: &Path) -> Option<DvdIfoStreams> {
    let disc = crate::video_ext::dvd_disc_root(vob)?;
    let vts_dir = crate::video_ext::dvd_video_ts_dir(&disc)?;
    let vts_id = vts_id_from_path(vob)?;
    let ifo = vts_dir.join(format!("VTS_{vts_id:02}_0.IFO"));
    streams_from_vts_ifo(&ifo)
}

pub fn streams_from_vts_ifo(ifo_path: &Path) -> Option<DvdIfoStreams> {
    let buf = IfoBuf::load(ifo_path)?;
    parse_streams(&buf)
}

fn parse_streams(buf: &IfoBuf) -> Option<DvdIfoStreams> {
    if buf.len() <= SUB_OFF + SUBP_ATTR_SIZE {
        return None;
    }
    let nr_audio = buf.byte(NR_AUDIO_OFF) as usize;
    let nr_sub = buf.byte(NR_SUB_OFF) as usize;
    if nr_audio > MAX_AUDIO || nr_sub > MAX_SUB {
        return None;
    }
    let audio_end = AUDIO_OFF.checked_add(nr_audio.saturating_mul(AUDIO_ATTR_SIZE))?;
    let sub_end = SUB_OFF.checked_add(nr_sub.saturating_mul(SUBP_ATTR_SIZE))?;
    if audio_end > buf.len() || sub_end > buf.len() {
        return None;
    }
    let mut audio = Vec::with_capacity(nr_audio);
    for slot in 0..nr_audio {
        let off = AUDIO_OFF + slot * AUDIO_ATTR_SIZE;
        let Some(raw) = buf.slice(off, AUDIO_ATTR_SIZE) else {
            break;
        };
        if let Some(row) = parse_audio_attr(raw, slot as u8) {
            audio.push(row);
        }
    }
    let mut sub = Vec::with_capacity(nr_sub);
    for slot in 0..nr_sub {
        let off = SUB_OFF + slot * SUBP_ATTR_SIZE;
        let Some(raw) = buf.slice(off, SUBP_ATTR_SIZE) else {
            break;
        };
        if let Some(row) = parse_subp_attr(raw, slot as u8) {
            sub.push(row);
        }
    }
    Some(DvdIfoStreams { audio, sub })
}

pub(super) fn parse_audio_attr(raw: &[u8], slot: u8) -> Option<DvdIfoAudio> {
    if raw.len() < AUDIO_ATTR_SIZE {
        return None;
    }
    let (format, lang_type, lang_code, ch_bits) = read_audio_attr(raw)?;
    let channels = ch_bits.saturating_add(1);
    if format == 0 && lang_type == 0 && channels == 1 && lang_code == 0 {
        return None;
    }
    let lang = if lang_type == 1 {
        lang_from_code(lang_code)
    } else {
        String::new()
    };
    let (codec_key, format_label) = audio_format_label(format)?;
    let ch_label = channel_label(channels);
    let label = compose_label(&lang, format_label, ch_label);
    Some(DvdIfoAudio {
        slot,
        lang,
        channels,
        label,
        codec_key,
    })
}

pub(super) fn parse_subp_attr(raw: &[u8], slot: u8) -> Option<DvdIfoSub> {
    if raw.len() < SUBP_ATTR_SIZE {
        return None;
    }
    let (typ, lang_code) = read_subp_attr(raw)?;
    let lang = if typ == 1 {
        lang_from_code(lang_code)
    } else {
        String::new()
    };
    if lang.is_empty() && typ == 0 {
        return None;
    }
    let label = if lang.is_empty() {
        format!("Subtitle {}", slot + 1)
    } else {
        lang.clone()
    };
    Some(DvdIfoSub { slot, lang, label })
}

fn lang_from_code(code: u16) -> String {
    let hi = (code >> 8) as u8;
    let lo = (code & 0xff) as u8;
    if hi.is_ascii_alphabetic() && lo.is_ascii_alphabetic() {
        format!("{}{}", hi as char, lo as char).to_lowercase()
    } else {
        String::new()
    }
}

fn audio_format_label(format: u8) -> Option<(&'static str, &'static str)> {
    Some(match format {
        0 => ("ac3", "AC-3"),
        2 => ("mpeg1", "MPEG-1"),
        3 => ("mpeg2", "MPEG-2"),
        4 => ("lpcm", "LPCM"),
        6 => ("dts", "DTS"),
        _ => return None,
    })
}

fn channel_label(channels: u8) -> &'static str {
    match channels {
        0 => "unknown",
        1 => "mono",
        2 => "stereo",
        5 | 6 => "5.1",
        _ => "surround",
    }
}

fn compose_label(lang: &str, format: &str, channels: &str) -> String {
    if lang.is_empty() {
        return format!("{format} {channels}");
    }
    format!("{lang} · {format} {channels}")
}

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

#[cfg(test)]
#[path = "streams_tests.rs"]
mod tests;
