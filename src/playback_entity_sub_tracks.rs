// DVD subtitle slot → mpv sid helpers (included by `playback_entity_tracks.rs`).

fn sub_stream_src_id(n: &TrackNode) -> Option<i64> {
    n.src_id.or(n.demuxer_src_id)
}

fn mpv_sub_track_metas(mpv: &Mpv) -> Vec<crate::dvd_ifo_parse::MpvSubTrackMeta> {
    track_nodes(mpv)
        .into_iter()
        .filter(|n| n.kind == "sub")
        .map(|n| crate::dvd_ifo_parse::MpvSubTrackMeta {
            id: n.id,
            src_id: sub_stream_src_id(&n),
            lang: n.lang,
        })
        .collect()
}

fn mpv_sid_for_slot(mpv: &Mpv, ifo: &crate::dvd_ifo_parse::DvdIfoStreams, slot: u8) -> Option<i64> {
    crate::dvd_ifo_parse::mpv_sub_id_for_ifo_slot(&ifo.sub, &mpv_sub_track_metas(mpv), slot)
}
