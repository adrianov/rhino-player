//! VTS IFO audio / subpicture stream attributes (`vtsi_mat_t`), aligned with libdvdread layout.

use std::path::Path;

use super::buf::IfoBuf;
use super::vts_id_from_path;

// `offsetof(vtsi_mat_t, …)` from libdvdread 7.x (Homebrew); stable for VTS_xx_0.IFO.
const NR_AUDIO_OFF: usize = 515;
const AUDIO_OFF: usize = 516;
const NR_SUB_OFF: usize = 597;
const SUB_OFF: usize = 598;
pub(super) const AUDIO_ATTR_SIZE: usize = 8;
pub(super) const SUBP_ATTR_SIZE: usize = 6;
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
    streams_from_vts_ifo(&vts_dir.join(format!("VTS_{vts_id:02}_0.IFO")))
}

pub fn streams_from_vts_ifo(ifo_path: &Path) -> Option<DvdIfoStreams> {
    let buf = IfoBuf::load(ifo_path)?;
    parse_streams(&buf)
}

fn parse_streams(buf: &IfoBuf) -> Option<DvdIfoStreams> {
    if buf.len() <= SUB_OFF + SUBP_ATTR_SIZE {
        return None;
    }
    let (nr_audio, nr_sub) = stream_counts(buf)?;
    Some(DvdIfoStreams {
        audio: parse_audio_rows(buf, nr_audio),
        sub: parse_sub_rows(buf, nr_sub),
    })
}

fn stream_counts(buf: &IfoBuf) -> Option<(usize, usize)> {
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
    Some((nr_audio, nr_sub))
}

fn parse_audio_rows(buf: &IfoBuf, nr_audio: usize) -> Vec<DvdIfoAudio> {
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
    audio
}

fn parse_sub_rows(buf: &IfoBuf, nr_sub: usize) -> Vec<DvdIfoSub> {
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
    sub
}

#[path = "streams_attrs.rs"]
mod attrs;

use attrs::{parse_audio_attr, parse_subp_attr};

#[path = "streams_matchers.rs"]
mod matchers;

pub use matchers::{
    audio_slot_for_meta, match_audio_label, match_sub_label, sub_slot_for_src_id, MpvTrackMeta,
};
#[cfg(test)]
#[path = "streams_tests.rs"]
mod tests;
