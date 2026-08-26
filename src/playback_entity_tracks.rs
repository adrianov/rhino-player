// Title-set audio/sub menus: IFO lists for DVD entities, mpv track-list otherwise.

use std::path::{Path, PathBuf};

use libmpv2::Mpv;
use serde::Deserialize;

use super::PlaybackEntity;

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
    #[serde(default, rename = "demux-channels")]
    demux_channels: Option<String>,
    #[serde(default)]
    forced: bool,
    #[serde(default, rename = "hearing-impaired")]
    hearing_impaired: bool,
    #[serde(default, rename = "visual-impaired")]
    visual_impaired: bool,
    #[serde(default)]
    default: bool,
}

impl PlaybackEntity {
    /// `VTS_xx_0.IFO` stream list for the open chapter's title set (same on every chapter of that set).
    #[must_use]
    pub fn title_set_streams(&self, chapter: &Path) -> Option<crate::dvd_ifo_parse::DvdIfoStreams> {
        if !matches!(self.kind, super::PlaybackEntityKind::DvdTitle { .. }) {
            return None;
        }
        crate::dvd_ifo_parse::ifo_streams_for_vob(chapter)
    }
}

/// Resolve entity + open path from mpv (`path` when local, else `shell` for `bd://` / disc trees).
#[must_use]
pub fn entity_from_mpv(mpv: &Mpv, shell: Option<&Path>) -> Option<(PlaybackEntity, PathBuf)> {
    let path = crate::media_probe::shell_media_path(mpv, shell)?;
    Some((PlaybackEntity::resolve(&path), path))
}

fn track_nodes(mpv: &Mpv) -> Vec<TrackNode> {
    let json = match mpv.get_property::<String>("track-list") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    serde_json::from_str(&json).unwrap_or_default()
}

/// Trimmed non-empty string view, or `None`.
fn trimmed(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

fn line_label(id: i64, title: Option<String>, lang: Option<String>, ifo: Option<&str>) -> String {
    if let Some(s) = trimmed(ifo) {
        return s.to_string();
    }
    let t = trimmed(title.as_deref());
    let l = trimmed(lang.as_deref());
    match (t, l) {
        (Some(a), Some(b)) => format!("{a} – {b}"),
        (Some(s), None) | (None, Some(s)) => s.to_string(),
        (None, None) => format!("Track {id}"),
    }
}

/// First subtitle track node plus its sub-only index, by mpv track id.
fn find_sub_track(nodes: &[TrackNode], sid: i64) -> Option<(&TrackNode, usize)> {
    let sub_nodes: Vec<&TrackNode> = nodes.iter().filter(|n| n.kind == "sub").collect();
    let idx = sub_nodes.iter().position(|n| n.id == sid)?;
    Some((sub_nodes[idx], idx))
}

include!("playback_entity_tracks_audio.rs");

include!("playback_entity_sub_tracks.rs");

/// Map current mpv `sid` to a title-set IFO sub slot (DVD only).
#[must_use]
pub fn sub_ifo_slot_for_sid(
    mpv: &Mpv,
    entity: &PlaybackEntity,
    sid: i64,
    shell: Option<&Path>,
) -> Option<u8> {
    let chapter = crate::media_probe::shell_media_path(mpv, shell)?;
    let ifo = entity.title_set_streams(&chapter)?;
    let nodes = track_nodes(mpv);
    let (n, idx) = find_sub_track(&nodes, sid)?;
    crate::dvd_ifo_parse::sub_slot_for_src_id(&ifo.sub, sub_stream_src_id(n), idx)
}

/// Resolve menu row → mpv `sid` on the open chapter.
#[must_use]
pub fn resolve_sub_mpv_id(
    mpv: &Mpv,
    entity: &PlaybackEntity,
    mpv_id: i64,
    ifo_slot: Option<u8>,
    shell: Option<&Path>,
) -> Option<i64> {
    let nodes = track_nodes(mpv);
    let sub_ids: Vec<i64> = nodes
        .iter()
        .filter(|n| n.kind == "sub")
        .map(|n| n.id)
        .collect();
    if mpv_id > 0 && sub_ids.contains(&mpv_id) {
        return Some(mpv_id);
    }
    let slot = ifo_slot?;
    let chapter = crate::media_probe::shell_media_path(mpv, shell)?;
    let ifo = entity.title_set_streams(&chapter)?;
    mpv_sid_for_slot(&nodes, &ifo, slot)
}

/// Whether the entity exposes title-set subtitle streams (IFO or mpv).
#[must_use]
pub fn entity_has_subtitles(mpv: &Mpv, shell: Option<&Path>) -> bool {
    if !sub_menu_rows(mpv, shell).is_empty() {
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
