// Title-set audio menus: IFO slot mapping and popover rows for the open entity
// (included by `playback_entity_tracks.rs`, which owns the shared track-node helpers).

fn audio_stream_src_id(n: &TrackNode) -> Option<i64> {
    n.src_id.or(n.demuxer_src_id)
}

fn mpv_aid_for_slot(
    nodes: &[TrackNode],
    ifo: &crate::dvd_ifo_parse::DvdIfoStreams,
    slot: u8,
) -> Option<i64> {
    for n in nodes {
        if n.kind != "audio" {
            continue;
        }
        let meta = crate::dvd_ifo_parse::MpvTrackMeta {
            src_id: audio_stream_src_id(n),
            codec: n.codec.as_deref(),
            demux_channels: n.demux_channel_count,
        };
        if crate::dvd_ifo_parse::audio_slot_for_meta(&ifo.audio, meta) == Some(slot) {
            return Some(n.id);
        }
    }
    None
}

fn audio_ifo_slot_for_aid_nodes(
    nodes: &[TrackNode],
    ifo: &crate::dvd_ifo_parse::DvdIfoStreams,
    aid: i64,
) -> Option<u8> {
    let n = nodes.iter().find(|n| n.kind == "audio" && n.id == aid)?;
    let meta = crate::dvd_ifo_parse::MpvTrackMeta {
        src_id: audio_stream_src_id(n),
        codec: n.codec.as_deref(),
        demux_channels: n.demux_channel_count,
    };
    crate::dvd_ifo_parse::audio_slot_for_meta(&ifo.audio, meta)
}

/// Map current mpv `aid` to a title-set IFO audio slot (DVD only).
#[must_use]
pub fn audio_ifo_slot_for_aid(
    mpv: &Mpv,
    entity: &PlaybackEntity,
    aid: i64,
    shell: Option<&Path>,
) -> Option<u8> {
    let chapter = crate::media_probe::shell_media_path(mpv, shell)?;
    let ifo = entity.title_set_streams(&chapter)?;
    audio_ifo_slot_for_aid_nodes(&track_nodes(mpv), &ifo, aid)
}

/// Resolve menu row → mpv `aid` on the open chapter.
#[must_use]
pub fn resolve_audio_mpv_id(
    mpv: &Mpv,
    entity: &PlaybackEntity,
    row: &AudioMenuRow,
    shell: Option<&Path>,
) -> Option<i64> {
    if row.mpv_id > 0 {
        return Some(row.mpv_id);
    }
    let slot = row.ifo_slot?;
    let chapter = crate::media_probe::shell_media_path(mpv, shell)?;
    let ifo = entity.title_set_streams(&chapter)?;
    mpv_aid_for_slot(&track_nodes(mpv), &ifo, slot)
}

fn ifo_audio_rows(
    nodes: &[TrackNode],
    ifo: &crate::dvd_ifo_parse::DvdIfoStreams,
) -> Vec<AudioMenuRow> {
    ifo.audio
        .iter()
        .map(|a| AudioMenuRow {
            mpv_id: mpv_aid_for_slot(nodes, ifo, a.slot).unwrap_or(-1),
            label: a.label.clone(),
            ifo_slot: Some(a.slot),
        })
        .collect()
}

fn mpv_audio_label_for_node(n: &TrackNode, ifo: Option<&str>) -> String {
    if let Some(s) = trimmed(ifo) {
        return s.to_string();
    }
    let rich = crate::track_menu_label::mpv_audio_label(
        n.lang.as_deref(),
        n.title.as_deref(),
        n.codec.as_deref(),
        n.demux_channel_count,
        n.demux_channels.as_deref(),
    );
    if !rich.is_empty() {
        return rich;
    }
    line_label(n.id, n.title.clone(), n.lang.clone(), None)
}

fn matched_audio_label(
    s: &crate::dvd_ifo_parse::DvdIfoStreams,
    n: &TrackNode,
    used: &mut [bool],
) -> Option<String> {
    crate::dvd_ifo_parse::match_audio_label(
        &s.audio,
        crate::dvd_ifo_parse::MpvTrackMeta {
            src_id: audio_stream_src_id(n),
            codec: n.codec.as_deref(),
            demux_channels: n.demux_channel_count,
        },
        used,
    )
}

fn apply_audio_label_disambiguation(rows: &mut [AudioMenuRow]) {
    let mut labels: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
    crate::track_menu_label::disambiguate_labels(&mut labels);
    for (row, label) in rows.iter_mut().zip(labels) {
        row.label = label;
    }
}

fn mpv_audio_rows(
    nodes: &[TrackNode],
    ifo: Option<&crate::dvd_ifo_parse::DvdIfoStreams>,
) -> Vec<AudioMenuRow> {
    let mut used = ifo.map(|s| vec![false; s.audio.len()]).unwrap_or_default();
    let mut v = vec![];
    for n in nodes {
        if n.kind != "audio" {
            continue;
        }
        let ifo_label = ifo.and_then(|s| matched_audio_label(s, n, &mut used));
        v.push(AudioMenuRow {
            mpv_id: n.id,
            label: mpv_audio_label_for_node(n, ifo_label.as_deref()),
            ifo_slot: None,
        });
    }
    apply_audio_label_disambiguation(&mut v);
    v
}

/// Sound popover rows for the current entity (IFO title-set list on DVD).
#[must_use]
pub fn audio_menu_rows(mpv: &Mpv, shell: Option<&Path>) -> Vec<AudioMenuRow> {
    let Some((entity, chapter)) = entity_from_mpv(mpv, shell) else {
        return vec![];
    };
    let nodes = track_nodes(mpv);
    let ifo = entity.title_set_streams(&chapter);
    if let Some(ifo) = &ifo {
        if !ifo.audio.is_empty() {
            return ifo_audio_rows(&nodes, ifo);
        }
    }
    mpv_audio_rows(&nodes, ifo.as_ref())
}
