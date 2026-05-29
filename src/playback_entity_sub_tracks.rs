// DVD subtitle slot → mpv sid helpers (included by `playback_entity_tracks.rs`).

fn sub_stream_src_id(n: &TrackNode) -> Option<i64> {
    n.src_id.or(n.demuxer_src_id)
}

fn mpv_sub_track_metas(nodes: &[TrackNode]) -> Vec<crate::dvd_ifo_parse::MpvSubTrackMeta> {
    nodes
        .iter()
        .filter(|n| n.kind == "sub")
        .map(|n| crate::dvd_ifo_parse::MpvSubTrackMeta {
            id: n.id,
            src_id: sub_stream_src_id(n),
            lang: n.lang.clone(),
        })
        .collect()
}

fn mpv_sid_for_slot(nodes: &[TrackNode], ifo: &crate::dvd_ifo_parse::DvdIfoStreams, slot: u8) -> Option<i64> {
    crate::dvd_ifo_parse::mpv_sub_id_for_ifo_slot(&ifo.sub, &mpv_sub_track_metas(nodes), slot)
}

fn ifo_sub_rows(nodes: &[TrackNode], ifo: &crate::dvd_ifo_parse::DvdIfoStreams) -> Vec<SubMenuRow> {
    let metas = mpv_sub_track_metas(nodes);
    ifo.sub
        .iter()
        .map(|s| SubMenuRow {
            mpv_id: crate::dvd_ifo_parse::mpv_sub_id_for_ifo_slot(&ifo.sub, &metas, s.slot).unwrap_or(-1),
            label: s.label.clone(),
            lang: s.lang.clone(),
            ifo_slot: Some(s.slot),
        })
        .collect()
}

fn mpv_sub_label_for_node(n: &TrackNode, ifo: Option<&str>) -> String {
    if let Some(s) = ifo.map(str::trim).filter(|s| !s.is_empty()) {
        return s.to_string();
    }
    let rich = crate::track_menu_label::mpv_sub_label(
        n.lang.as_deref(),
        n.title.as_deref(),
        n.codec.as_deref(),
        n.forced,
        n.hearing_impaired,
        n.visual_impaired,
        n.default,
    );
    if !rich.is_empty() {
        return rich;
    }
    line_label(n.id, n.title.clone(), n.lang.clone(), None)
}

fn mpv_sub_rows(nodes: &[TrackNode], ifo: Option<&crate::dvd_ifo_parse::DvdIfoStreams>) -> Vec<SubMenuRow> {
    let mut used = ifo
        .map(|s| vec![false; s.sub.len()])
        .unwrap_or_default();
    let mut v = vec![];
    for n in nodes {
        if n.kind != "sub" {
            continue;
        }
        let ifo_label = ifo.and_then(|s| {
            let slot_byte =
                crate::dvd_ifo_parse::sub_slot_for_src_id(&s.sub, sub_stream_src_id(n), v.len())?;
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
            label: mpv_sub_label_for_node(n, ifo_label.as_deref()),
            lang: if lang.is_empty() {
                ifo_label.unwrap_or_default()
            } else {
                lang
            },
            ifo_slot: None,
        });
    }
    let mut labels: Vec<String> = v.iter().map(|r| r.label.clone()).collect();
    apply_label_disambiguation(&mut labels);
    for (row, label) in v.iter_mut().zip(labels) {
        row.label = label;
    }
    v
}

fn sub_codecs_from_nodes(nodes: &[TrackNode]) -> Vec<(i64, String)> {
    nodes
        .iter()
        .filter(|n| n.kind == "sub")
        .map(|n| (n.id, n.codec.clone().unwrap_or_default()))
        .collect()
}

/// Subtitle menu rows and `(id, codec)` pairs from one `track-list` parse.
#[must_use]
pub fn sub_menu_snapshot(mpv: &Mpv, shell: Option<&Path>) -> (Vec<SubMenuRow>, Vec<(i64, String)>) {
    let Some((entity, chapter)) = entity_from_mpv(mpv, shell) else {
        return (vec![], vec![]);
    };
    let nodes = track_nodes(mpv);
    let codecs = sub_codecs_from_nodes(&nodes);
    let ifo = entity.title_set_streams(&chapter);
    let rows = match ifo.as_ref() {
        Some(ifo) if !ifo.sub.is_empty() => ifo_sub_rows(&nodes, ifo),
        Some(ifo) => mpv_sub_rows(&nodes, Some(ifo)),
        None => mpv_sub_rows(&nodes, None),
    };
    (rows, codecs)
}

/// Subtitles popover rows for the current entity (IFO title-set list on DVD).
#[must_use]
pub fn sub_menu_rows(mpv: &Mpv, shell: Option<&Path>) -> Vec<SubMenuRow> {
    sub_menu_snapshot(mpv, shell).0
}
