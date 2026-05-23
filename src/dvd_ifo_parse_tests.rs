use super::*;
use super::time::dvdtime_to_sec;
use std::path::Path;

#[test]
fn vts_and_vob_part_from_stem() {
    let p = Path::new("/d/VIDEO_TS/VTS_02_3.VOB");
    assert_eq!(vts_id_from_path(p), Some(2));
    assert_eq!(crate::dvd_entity::vob_part_id(p), Some(3));
}

#[test]
fn dvdtime_pal_25fps() {
    assert!((dvdtime_to_sec(&[0, 0, 10, 0x41]) - 10.04).abs() < 0.01);
}

/// Skips when the local sample rip is not mounted.
#[test]
fn timeline_from_mounted_dvd_sample() {
    let vob = Path::new(
        "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD1/Video_ts/VTS_02_1.VOB",
    );
    if !vob.is_file() {
        return;
    }
    let disc = vob.parent().unwrap().parent().unwrap();
    let main = main_title_from_disc(disc).expect("VIDEO_TS.IFO main title");
    assert_eq!(main.0, 2, "expected VTS_02 main feature");
    let tl = timeline_from_vob(vob).expect("VTS_02_0.IFO timeline");
    assert!(!tl.vob_secs.is_empty());
    let total: f64 = tl.vob_secs.iter().map(|(_, s)| s).sum();
    assert!(total > 60.0, "title should be longer than one chapter");
}
