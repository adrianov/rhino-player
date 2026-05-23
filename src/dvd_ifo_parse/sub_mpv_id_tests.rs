use super::{mpv_sub_id_for_ifo_slot, MpvSubTrackMeta};
use super::super::streams::DvdIfoSub;

fn ru_sub(slot: u8) -> DvdIfoSub {
    DvdIfoSub {
        slot,
        lang: "ru".into(),
        label: "ru".into(),
    }
}

fn en_sub(slot: u8) -> DvdIfoSub {
    DvdIfoSub {
        slot,
        lang: "en".into(),
        label: "en".into(),
    }
}

#[test]
fn mpv_sub_id_for_ifo_slot_by_src_id() {
    let ifo = [ru_sub(0)];
    let tracks = [MpvSubTrackMeta {
        id: 2,
        src_id: Some(0x20),
        lang: Some("ru".into()),
    }];
    assert_eq!(mpv_sub_id_for_ifo_slot(&ifo, &tracks, 0), Some(2));
}

#[test]
fn mpv_sub_id_for_ifo_slot_by_list_index() {
    let ifo = [ru_sub(0), en_sub(1)];
    let tracks = [
        MpvSubTrackMeta {
            id: 3,
            src_id: None,
            lang: Some("ru".into()),
        },
        MpvSubTrackMeta {
            id: 4,
            src_id: None,
            lang: Some("en".into()),
        },
    ];
    assert_eq!(mpv_sub_id_for_ifo_slot(&ifo, &tracks, 1), Some(4));
}

#[test]
fn mpv_sub_id_for_ifo_slot_by_lang() {
    let ifo = [ru_sub(0)];
    let tracks = [MpvSubTrackMeta {
        id: 5,
        src_id: None,
        lang: Some("ru".into()),
    }];
    assert_eq!(mpv_sub_id_for_ifo_slot(&ifo, &tracks, 0), Some(5));
}
