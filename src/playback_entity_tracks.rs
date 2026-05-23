// Title-set audio/sub menus: IFO lists for DVD entities, mpv track-list otherwise.

use std::path::PathBuf;

use libmpv2::Mpv;
use serde::Deserialize;

use super::{PlaybackEntity, PlaybackEntityKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioMenuRow {
    pub mpv_id: i64,
    pub label: String,
    pub ifo_slot: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubMenuRow {
    pub mpv_id: i64,
    pub label: String,
    pub lang: String,
    pub ifo_slot: Option<u8>,
}

#[derive(Deserialize)]
struct TrackNode {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "src-id")]
    src_id: Option<i64>,
    #[serde(default, rename = "demuxer-src-id")]
    demuxer_src_id: Option<i64>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default, rename = "demux-channel-count")]
    demux_channel_count: Option<i64>,
}

impl PlaybackEntity {
    /// `VTS_xx_0.IFO` stream list for a multi-part DVD title (same on every chapter).
    #[must_use]
    pub fn title_set_streams(&self) -> Option<crate::dvd_ifo_parse::DvdIfoStreams> {
        let PlaybackEntityKind::DvdTitle { chapters, .. } = &self.kind else {
            return None;
        };
        let probe = chapters.first()?;
        crate::dvd_ifo_parse::ifo_streams_for_vob(probe)
    }
}

/// Resolve entity + local path from the playback engine.
#[must_use]
pub fn entity_from_mpv(mpv: &Mpv) -> Option<(PlaybackEntity, PathBuf)> {
    let path = crate::media_probe::local_file_from_mpv(mpv)?;
    Some((PlaybackEntity::resolve(&path), path))
}

fn track_nodes(mpv: &Mpv) -> Vec<TrackNode> {
    let json = match mpv.get_property::<String>("track-list") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    serde_json::from_str(&json).unwrap_or_default()
}

fn line_label(id: i64, title: Option<String>, lang: Option<String>, ifo: Option<&str>) -> String {
    if let Some(s) = ifo.map(str::trim).filter(|s| !s.is_empty()) {
        return s.to_string();
    }
    let t = title.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let l = lang.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let (Some(a), Some(b)) = (t, l) {
        return format!("{a} – {b}");
    }
    if let Some(s) = t.or(l) {
        return s.to_string();
    }
    format!("Track {id}")
}

fn mpv_aid_for_slot(mpv: &Mpv, ifo: &crate::dvd_ifo_parse::DvdIfoStreams, slot: u8) -> Option<i64> {
    for n in track_nodes(mpv) {
        if n.kind != "audio" {
            continue;
        }
        let meta = crate::dvd_ifo_parse::MpvTrackMeta {
            src_id: n.src_id,
            codec: n.codec.as_deref(),
            demux_channels: n.demux_channel_count,
        };
        if crate::dvd_ifo_parse::audio_slot_for_meta(&ifo.audio, meta) == Some(slot) {
            return Some(n.id);
        }
    }
    None
}

/// Map current mpv `aid` to a title-set IFO audio slot (DVD only).
#[must_use]
pub fn audio_ifo_slot_for_aid(mpv: &Mpv, entity: &PlaybackEntity, aid: i64) -> Option<u8> {
    let ifo = entity.title_set_streams()?;
    let n = track_nodes(mpv)
        .into_iter()
        .find(|n| n.kind == "audio" && n.id == aid)?;
    let meta = crate::dvd_ifo_parse::MpvTrackMeta {
        src_id: n.src_id,
        codec: n.codec.as_deref(),
        demux_channels: n.demux_channel_count,
    };
    crate::dvd_ifo_parse::audio_slot_for_meta(&ifo.audio, meta)
}

/// Resolve menu row → mpv `aid` on the open chapter.
#[must_use]
pub fn resolve_audio_mpv_id(mpv: &Mpv, entity: &PlaybackEntity, row: &AudioMenuRow) -> Option<i64> {
    if row.mpv_id > 0 {
        return Some(row.mpv_id);
    }
    let slot = row.ifo_slot?;
    let ifo = entity.title_set_streams()?;
    mpv_aid_for_slot(mpv, &ifo, slot)
}

fn ifo_audio_rows(mpv: &Mpv, ifo: &crate::dvd_ifo_parse::DvdIfoStreams) -> Vec<AudioMenuRow> {
    ifo.audio
        .iter()
        .map(|a| AudioMenuRow {
            mpv_id: mpv_aid_for_slot(mpv, ifo, a.slot).unwrap_or(-1),
            label: a.label.clone(),
            ifo_slot: Some(a.slot),
        })
        .collect()
}

fn mpv_audio_rows(mpv: &Mpv, ifo: Option<&crate::dvd_ifo_parse::DvdIfoStreams>) -> Vec<AudioMenuRow> {
    let mut used = ifo
        .map(|s| vec![false; s.audio.len()])
        .unwrap_or_default();
    let mut v = vec![];
    for n in track_nodes(mpv) {
        if n.kind != "audio" {
            continue;
        }
        let ifo_label = ifo.and_then(|s| {
            crate::dvd_ifo_parse::match_audio_label(
                &s.audio,
                crate::dvd_ifo_parse::MpvTrackMeta {
                    src_id: n.src_id,
                    codec: n.codec.as_deref(),
                    demux_channels: n.demux_channel_count,
                },
                &mut used,
            )
        });
        v.push(AudioMenuRow {
            mpv_id: n.id,
            label: line_label(n.id, n.title, n.lang, ifo_label.as_deref()),
            ifo_slot: None,
        });
    }
    v
}

/// Sound popover rows for the current entity (IFO title-set list on DVD).
#[must_use]
pub fn audio_menu_rows(mpv: &Mpv) -> Vec<AudioMenuRow> {
    let Some((entity, _)) = entity_from_mpv(mpv) else {
        return vec![];
    };
    if let Some(ifo) = entity.title_set_streams() {
        if !ifo.audio.is_empty() {
            return ifo_audio_rows(mpv, &ifo);
        }
    }
    mpv_audio_rows(mpv, entity.title_set_streams().as_ref())
}

include!("playback_entity_sub_tracks.rs");

/// Map current mpv `sid` to a title-set IFO sub slot (DVD only).
#[must_use]
pub fn sub_ifo_slot_for_sid(mpv: &Mpv, entity: &PlaybackEntity, sid: i64) -> Option<u8> {
    let ifo = entity.title_set_streams()?;
    let nodes: Vec<_> = track_nodes(mpv)
        .into_iter()
        .filter(|n| n.kind == "sub")
        .collect();
    let n = nodes.iter().find(|n| n.id == sid)?;
    let idx = nodes.iter().position(|x| x.id == sid)?;
    crate::dvd_ifo_parse::sub_slot_for_src_id(&ifo.sub, sub_stream_src_id(n), idx)
}

/// Resolve menu row → mpv `sid` on the open chapter.
#[must_use]
pub fn resolve_sub_mpv_id(mpv: &Mpv, entity: &PlaybackEntity, mpv_id: i64, ifo_slot: Option<u8>) -> Option<i64> {
    let sub_ids: Vec<i64> = track_nodes(mpv)
        .into_iter()
        .filter(|n| n.kind == "sub")
        .map(|n| n.id)
        .collect();
    if mpv_id > 0 && sub_ids.contains(&mpv_id) {
        return Some(mpv_id);
    }
    let slot = ifo_slot?;
    let ifo = entity.title_set_streams()?;
    mpv_sid_for_slot(mpv, &ifo, slot)
}

fn ifo_sub_rows(mpv: &Mpv, ifo: &crate::dvd_ifo_parse::DvdIfoStreams) -> Vec<SubMenuRow> {
    ifo.sub
        .iter()
        .map(|s| SubMenuRow {
            mpv_id: mpv_sid_for_slot(mpv, ifo, s.slot).unwrap_or(-1),
            label: s.label.clone(),
            lang: s.lang.clone(),
            ifo_slot: Some(s.slot),
        })
        .collect()
}

fn mpv_sub_rows(mpv: &Mpv, ifo: Option<&crate::dvd_ifo_parse::DvdIfoStreams>) -> Vec<SubMenuRow> {
    let mut used = ifo
        .map(|s| vec![false; s.sub.len()])
        .unwrap_or_default();
    let mut v = vec![];
    for n in track_nodes(mpv) {
        if n.kind != "sub" {
            continue;
        }
        let ifo_label = ifo.and_then(|s| {
            let slot_byte =
                crate::dvd_ifo_parse::sub_slot_for_src_id(&s.sub, sub_stream_src_id(&n), v.len())?;
            let idx = s
                .sub
                .iter()
                .position(|r| r.slot == slot_byte)
                .unwrap_or(v.len());
            crate::dvd_ifo_parse::match_sub_label(&s.sub, idx, &mut used)
        });
        let lang = n
            .lang
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_string();
        v.push(SubMenuRow {
            mpv_id: n.id,
            label: line_label(n.id, n.title, n.lang.clone(), ifo_label.as_deref()),
            lang: if lang.is_empty() {
                ifo_label.unwrap_or_default()
            } else {
                lang
            },
            ifo_slot: None,
        });
    }
    v
}

/// Subtitles popover rows for the current entity (IFO title-set list on DVD).
#[must_use]
pub fn sub_menu_rows(mpv: &Mpv) -> Vec<SubMenuRow> {
    let Some((entity, _)) = entity_from_mpv(mpv) else {
        return vec![];
    };
    if let Some(ifo) = entity.title_set_streams() {
        if !ifo.sub.is_empty() {
            return ifo_sub_rows(mpv, &ifo);
        }
    }
    mpv_sub_rows(mpv, entity.title_set_streams().as_ref())
}

/// Whether the entity exposes title-set subtitle streams (IFO or mpv).
#[must_use]
pub fn entity_has_subtitles(mpv: &Mpv) -> bool {
    if !sub_menu_rows(mpv).is_empty() {
        return true;
    }
    let Ok(count) = mpv.get_property::<i64>("track-list/count") else {
        return false;
    };
    for i in 0..count.max(0) {
        let key = format!("track-list/{i}/type");
        if mpv.get_property::<String>(&key).is_ok_and(|s| s == "sub") {
            return true;
        }
    }
    false
}
