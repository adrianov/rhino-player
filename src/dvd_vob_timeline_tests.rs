#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn write_vob(dir: &std::path::Path, name: &str) {
        fs::write(dir.join(name), b"vob").expect("write");
    }

    #[test]
    fn global_pos_and_resolve_round_trip() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-tl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_01_2.VOB");
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let mut map = std::collections::HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        map.insert(p2.to_string_lossy().into_owned(), 50.0);
        let tl = DvdVobTimeline::from_chapter(&p1, &map, &p1, 100.0).expect("tl");
        assert!((tl.total_sec - 150.0).abs() < 1e-6);
        assert!((tl.global_pos(&p2, 10.0) - 110.0).abs() < 1e-6);
        let (idx, local) = tl.resolve_global(110.0);
        assert_eq!(idx, 1);
        assert!((local - 10.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn title_scope_excludes_other_vts_numbers() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-title-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p21 = vts.join("VTS_02_1.VOB");
        let list = crate::dvd_entity::list_title_vobs(&vts, &p21);
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|p| {
            crate::dvd_entity::vob_part_id(p).is_some()
        }));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn multi_chapter_uses_entity_total_over_single_live_chapter() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-tl-total-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let entity = crate::playback_entity::db_path_for(&p1);
        let mut map = std::collections::HashMap::new();
        map.insert(entity.to_string_lossy().into_owned(), 5000.0);
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        let tl = DvdVobTimeline::from_chapter(&p1, &map, &p1, 100.0).expect("tl");
        assert!((tl.total_sec - 5000.0).abs() < 1.0);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn fill_missing_keeps_known_chapter_lengths() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-fill-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 2000]).expect("write");
        fs::write(vts.join("VTS_02_3.VOB"), vec![0u8; 3000]).expect("write");
        fs::write(vts.join("VTS_02_4.VOB"), vec![0u8; 4000]).expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        let entity = crate::playback_entity::db_path_for(&p1);
        let mut map = HashMap::new();
        map.insert(entity.to_string_lossy().into_owned(), 4000.0);
        map.insert(p1.to_string_lossy().into_owned(), 600.0);
        map.insert(
            vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
            500.0,
        );
        let still = crate::dvd_entity::still_target_from_global(&p1, 1500.0, &map).expect("still");
        assert!(
            crate::video_ext::paths_same_file(&still.load, &p3),
            "expected vob3 got {}",
            still.load.display()
        );
        assert!((still.local_sec - 400.0).abs() < 2.0, "local={}", still.local_sec);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn multi_chapter_bootstraps_from_bytes_without_db_total() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-bytes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 2000]).expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let tl = DvdVobTimeline::from_chapter(&p1, &std::collections::HashMap::new(), &p1, 50.0)
            .expect("tl");
        assert!(tl.total_sec > 100.0);
        assert_eq!(tl.vobs.len(), 2);
        let _ = fs::remove_dir_all(&base);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn ifo_timeline_has_next_chapter_on_sample() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD1/VIDEO_TS/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let tl = DvdVobTimeline::from_chapter_ifo(vob).expect("ifo timeline");
        assert!(
            tl.vobs.len() >= 2,
            "main title should list multiple chapter VOBs"
        );
        assert!(tl.next_chapter_after(vob).is_some());
        let bar = crate::dvd_vob_timeline::DvdBarState::build(vob, 1080.0).expect("bar");
        assert!(
            bar.tl.vobs.len() >= 2,
            "bar build should expand on-disk chapters"
        );
        if !bar.tl.ptt_marks.is_empty() {
            let labels = bar.chapter_preview_labels();
            assert_eq!(
                labels.first().map(|(_, s)| s.as_str()),
                Some("Chapter 1")
            );
        }
    }

    #[test]
    fn resolve_picks_chapter_by_duration_window() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-win-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_01_2.VOB");
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let mut map = std::collections::HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        map.insert(p2.to_string_lossy().into_owned(), 40.0);
        let tl = DvdVobTimeline::from_chapter(&p1, &map, &p1, 100.0).expect("tl");
        let (idx, local) = tl.resolve_global(105.0);
        assert_eq!(idx, 1);
        assert!((local - 5.0).abs() < 1e-6);
        let (idx0, _) = tl.resolve_global(10.0);
        assert_eq!(idx0, 0);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn eof_continue_uses_live_tail_beyond_stored_chapter_dur() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-eof-tail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1102.3);
        map.insert(p2.to_string_lossy().into_owned(), 1100.0);
        let tl = DvdVobTimeline::from_chapter(&p1, &map, &p1, 1102.3).expect("tl");
        let (next, local, g) = tl
            .continue_after_vob_eof(&p1, 1104.78)
            .expect("continue");
        assert!(
            crate::video_ext::paths_same_file(&next, &p2),
            "expected vob2 got {}",
            next.display()
        );
        assert!(
            (local - 2.53).abs() < 0.1,
            "local spill into vob2, got {local}"
        );
        assert!((g - 1104.83).abs() < 0.1, "hold global {g}");
        let (_, local0, _) = tl.continue_after_vob_eof(&p1, 1102.3).expect("at stored end");
        assert!(local0 < 0.1, "stored end lands at next vob start");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn stale_total_still_lists_next_chapter() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-eof-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1105.0);
        let tl = DvdVobTimeline::from_chapter(&p1, &map, &p1, 1105.0).expect("tl");
        assert!(
            tl.next_chapter_after(&p1).is_some(),
            "second chapter must remain reachable when total equals first chapter only"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_labels_empty_for_single_chapter_vob() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-one-ch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let tl = DvdVobTimeline::from_chapter(&p1, &HashMap::new(), &p1, 100.0).expect("tl");
        assert_eq!(tl.vobs.len(), 1);
        assert!(tl.chapter_preview_labels().is_empty());
        let _ = fs::remove_dir_all(&base);
    }
}
