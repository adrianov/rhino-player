use super::*;
use std::path::Path;

#[test]
fn parse_sample_vts02_audio_bytes() {
    for (raw, slot, codec, ch) in [
        (&[0x04, 0xc5, 0x72, 0x75, 0, 0, 0, 0], 0, "ac3", 6),
        (&[0xc4, 0xc4, 0x72, 0x75, 0, 0, 0, 0], 1, "dts", 5),
        (&[0x04, 0xc0, 0x72, 0x75, 0, 0, 0, 0], 2, "ac3", 1),
    ] {
        assert_audio_row(raw, slot, codec, ch);
    }
    assert_exact_audio_labels();
}

fn assert_audio_row(raw: &[u8], slot: u8, codec: &str, ch: u8) {
    let row = parse_audio_attr(raw, slot).expect("audio row");
    assert_eq!(row.codec_key, codec);
    assert_eq!(row.channels, ch);
    assert_eq!(row.lang, "ru");
    assert!(row.label.contains("ru"));
}

fn assert_exact_audio_labels() {
    let ac3 = parse_audio_attr(&[0x04, 0xc5, 0x72, 0x75, 0, 0, 0, 0], 0).unwrap();
    assert_eq!(ac3.label, "ru · AC-3 5.1");
    let dts = parse_audio_attr(&[0xc4, 0xc4, 0x72, 0x75, 0, 0, 0, 0], 1).unwrap();
    assert_eq!(dts.label, "ru · DTS 5.1");
    let mono = parse_audio_attr(&[0x04, 0xc0, 0x72, 0x75, 0, 0, 0, 0], 2).unwrap();
    assert_eq!(mono.label, "ru · AC-3 mono");
}

#[test]
fn parse_sample_subp_attr() {
    let row = parse_subp_attr(&[0x01, 0, 0x72, 0x75, 0, 0], 0).expect("sub");
    assert_eq!(row.lang, "ru");
    assert_eq!(row.label, "ru");
}

#[test]
fn match_audio_by_codec_and_src_id() {
    let streams = [
        parse_audio_attr(&[0x04, 0xc5, 0x72, 0x75, 0, 0, 0, 0], 0).unwrap(),
        parse_audio_attr(&[0xc4, 0xc4, 0x72, 0x75, 0, 0, 0, 0], 1).unwrap(),
        parse_audio_attr(&[0x04, 0xc0, 0x72, 0x75, 0, 0, 0, 0], 2).unwrap(),
        parse_audio_attr(&[0x04, 0xc0, 0x72, 0x75, 0, 0, 0, 0], 3).unwrap(),
    ];
    let mut used = [false; 4];
    assert_eq!(
        matched_audio_label(&streams, 0x89, "dts", 6, &mut used),
        "ru · DTS 5.1"
    );
    assert_eq!(
        matched_audio_label(&streams, 0x80, "ac3", 6, &mut used),
        "ru · AC-3 5.1"
    );
    assert_eq!(
        matched_audio_label(&streams, 0x82, "ac3", 1, &mut used),
        "ru · AC-3 mono"
    );
    assert_eq!(
        matched_audio_label(&streams, 0x83, "ac3", 1, &mut used),
        "ru · AC-3 mono"
    );
}

fn matched_audio_label(
    streams: &[DvdIfoAudio],
    src_id: i64,
    codec: &str,
    demux_channels: i64,
    used: &mut [bool],
) -> String {
    match_audio_label(
        streams,
        MpvTrackMeta {
            src_id: Some(src_id),
            codec: Some(codec),
            demux_channels: Some(demux_channels),
        },
        used,
    )
    .unwrap()
}

#[test]
fn audio_slot_for_meta_by_src_id() {
    let streams = [
        parse_audio_attr(&[0x04, 0xc5, 0x72, 0x75, 0, 0, 0, 0], 0).unwrap(),
        parse_audio_attr(&[0xc4, 0xc4, 0x72, 0x75, 0, 0, 0, 0], 1).unwrap(),
    ];
    assert_eq!(
        audio_slot_for_meta(
            &streams,
            MpvTrackMeta {
                src_id: Some(0x89),
                codec: Some("dts"),
                demux_channels: Some(6),
            }
        ),
        Some(1)
    );
    assert_eq!(
        audio_slot_for_meta(
            &streams,
            MpvTrackMeta {
                src_id: Some(0x80),
                codec: Some("ac3"),
                demux_channels: Some(6),
            }
        ),
        Some(0)
    );
}

#[test]
fn sub_slot_for_src_id_maps_dvd_subpicture() {
    let streams = [parse_subp_attr(&[0x01, 0, 0x72, 0x75, 0, 0], 0).unwrap()];
    assert_eq!(sub_slot_for_src_id(&streams, Some(0x20), 0), Some(0));
    assert_eq!(sub_slot_for_src_id(&streams, None, 0), Some(0));
}

#[test]
fn streams_from_mounted_treasure_island_vts04() {
    let vob = Path::new("/Volumes/SanDisk/Torrents/TREASURE_ISLAND/VIDEO_TS/VTS_04_1.VOB");
    if !vob.is_file() {
        return;
    }
    let s = streams_from_vob(vob).expect("streams");
    assert_eq!(s.audio.len(), 4, "VTS_04 should expose four audio streams");
    assert!(s.audio.iter().any(|a| a.lang == "ru" && a.channels >= 5));
    assert!(s.audio.iter().any(|a| a.lang == "en"));
    assert!(s.audio.iter().any(|a| a.lang == "fr"));
    assert_eq!(s.sub.len(), 6);
}

#[test]
fn treasure_island_vts04_not_vts02_ifo() {
    let v04 = Path::new("/Volumes/SanDisk/Torrents/TREASURE_ISLAND/VIDEO_TS/VTS_04_1.VOB");
    let v02 = Path::new("/Volumes/SanDisk/Torrents/TREASURE_ISLAND/VIDEO_TS/VTS_02_1.VOB");
    if !v04.is_file() || !v02.is_file() {
        return;
    }
    let e = crate::playback_entity::PlaybackEntity::resolve(v04);
    let main = e.title_set_streams(v04).expect("vts04 ifo");
    let wrong = e.title_set_streams(v02).expect("vts02 ifo");
    assert_eq!(main.audio.len(), 4);
    assert_eq!(wrong.audio.len(), 1);
    assert_ne!(main, wrong);
}

#[test]
fn streams_from_mounted_dvd2_vts02() {
    let vob = Path::new(
        "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD2/Video_ts/VTS_02_1.VOB",
    );
    if !vob.is_file() {
        return;
    }
    let s = streams_from_vob(vob).expect("streams");
    assert_eq!(s.audio.len(), 4);
    assert!(s.audio.iter().all(|a| a.lang == "ru"));
    assert_eq!(s.sub.len(), 1);
    assert_eq!(s.sub[0].lang, "ru");
}
