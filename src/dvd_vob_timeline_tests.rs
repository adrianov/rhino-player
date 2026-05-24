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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, Some(&p1), 100.0).expect("tl");
        assert!((tl.total_sec - 150.0).abs() < 1e-6);
        assert!((tl.global_pos(&p2, 10.0) - 110.0).abs() < 1e-6);
        let (idx, local) = tl.resolve_global(110.0);
        assert_eq!(idx, 1);
        assert!((local - 10.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn feature_queue_includes_every_title_set_excludes_menu() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-feat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        write_vob(&vts, "VTS_03_1.VOB");
        write_vob(&vts, "VTS_03_2.VOB");
        let p21 = vts.join("VTS_02_1.VOB");
        let list = crate::dvd_entity::list_feature_vobs(&p21);
        assert_eq!(list.len(), 4);
        assert!(
            list.iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB"))
        );
        let title = crate::dvd_entity::vob_title_id(&p21);
        assert_eq!(
            list.iter()
                .filter(|p| crate::dvd_entity::vob_title_id(p) == title)
                .count(),
            2
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn multi_chapter_total_is_sum_of_vob_lengths() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-tl-total-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let mut map = std::collections::HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        map.insert(p2.to_string_lossy().into_owned(), 50.0);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, Some(&p1), 100.0).expect("tl");
        assert!((tl.total_sec - 150.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resume_maps_global_with_per_vob_durations() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-ent-res-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        let sizes = [100usize, 200, 300, 400];
        for (i, n) in sizes.iter().enumerate() {
            fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; *n]).expect("vob");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        map.insert(vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(), 200.0);
        map.insert(p3.to_string_lossy().into_owned(), 300.0);
        map.insert(vts.join("VTS_02_4.VOB").to_string_lossy().into_owned(), 400.0);
        let still = crate::dvd_entity::still_target_from_global(&p1, 350.0, &map).expect("still");
        assert!(crate::video_ext::paths_same_file(&still.load, &p3));
        assert!((still.local_sec - 50.0).abs() < 1.0, "local={}", still.local_sec);
        let still2 = crate::dvd_entity::still_target_from_global(&base, 350.0, &map).expect("disc");
        assert!(crate::video_ext::paths_same_file(&still2.load, &p3));
        assert!((still2.local_sec - 50.0).abs() < 1.0, "local2={}", still2.local_sec);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn map_durations_resolve_global() {
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
        map.insert(
            vts.join("VTS_02_3.VOB").to_string_lossy().into_owned(),
            400.0,
        );
        map.insert(
            vts.join("VTS_02_4.VOB").to_string_lossy().into_owned(),
            400.0,
        );
        let still = crate::dvd_entity::still_target_from_global(&p1, 1400.0, &map).expect("still");
        assert!(
            crate::video_ext::paths_same_file(&still.load, &p3),
            "expected vob3 got {}",
            still.load.display()
        );
        assert!((still.local_sec - 300.0).abs() < 2.0, "local={}", still.local_sec);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_chapter_dur_caps_at_next_mark() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-prev-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 100.0);
        map.insert(
            vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
            50.0,
        );
        let bar = DvdBarState::build(&p1, 100.0).expect("bar");
        let dur = preview_chapter_dur(&bar, 90.0, 0, 90.0, &p1, &map);
        assert!((dur - 100.0).abs() < 1e-6, "cap at ch2, got {dur}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn ifo_chapter_labels_scale_to_vob_total() {
        let ifo = crate::dvd_ifo_parse::IfoChapterMarks {
            mark_secs: vec![1000.0, 2000.0],
            title_sec: 4000.0,
        };
        let vob_total = 4200.0;
        let scale = vob_total / ifo.title_sec;
        let mut labels = vec![(0.0, "Chapter 1".to_string())];
        for (i, &m) in ifo.mark_secs.iter().enumerate() {
            labels.push((m * scale, format!("Chapter {}", i + 2)));
        }
        assert_eq!(labels.len(), 3);
        assert!((labels[1].0 - 1050.0).abs() < 1e-6);
    }

    #[test]
    fn no_guess_without_durs_or_mpv_probe() {
        crate::dvd_vob_mpv_probe::clear_probe_cache();
        let base = std::env::temp_dir().join(format!("rhino-dvd-noanchor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 2000]).expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let tl = DvdVobTimeline::from_title_vobs(&p1, &HashMap::new(), None, 0.0);
        assert!(tl.is_none(), "invalid .vob bytes must not invent a timeline");
        let _ = fs::remove_dir_all(&base);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd5_mpv_probe_fills_full_timeline() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD5/VIDEO_TS/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let list = crate::dvd_entity::list_feature_vobs(vob);
        assert_eq!(list.len(), 9, "DVD5 should queue VTS_02 and VTS_03 chapter files");
        assert!(
            list.iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB"))
        );
        crate::dvd_vob_mpv_probe::clear_probe_cache();
        let disc = crate::video_ext::dvd_disc_root(vob).expect("disc");
        let mut map = HashMap::new();
        map.insert(disc.to_string_lossy().into_owned(), 1131.1);
        let tl = DvdVobTimeline::from_title_vobs(vob, &map, Some(vob), 1129.0).expect("tl");
        assert!(
            tl.total_sec > 5000.0,
            "DVD5 bar should span all nine chapter files, got {:.1}s",
            tl.total_sec
        );
        let vts03 = vob.with_file_name("VTS_03_1.VOB");
        let (idx, _) = tl.resolve_global(tl.total_sec * 0.72);
        let target = tl.path_at(idx).expect("path");
        assert_eq!(
            crate::dvd_entity::vob_title_id(target),
            crate::dvd_entity::vob_title_id(&vts03),
            "seek target should reach VTS_03, got {:?}",
            target.file_name()
        );
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd4_lists_all_feature_vobs() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD4/Video_ts/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let list = crate::dvd_entity::list_feature_vobs(vob);
        assert!(
            list.len() >= 8,
            "DVD4 should queue VTS_02 and VTS_03 chapter files, got {}",
            list.len()
        );
        assert!(
            list.iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB"))
        );
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn vob_timeline_lists_on_disk_files() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD1/VIDEO_TS/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let bar = crate::dvd_vob_timeline::DvdBarState::build(vob, 1080.0).expect("bar");
        assert!(
            bar.tl.vobs.len() >= 2,
            "title should list every on-disk .vob in natural order"
        );
        assert!(bar.tl.next_chapter_after(vob).is_some());
        let labels = bar.chapter_preview_labels();
        if !labels.is_empty() {
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, Some(&p1), 100.0).expect("tl");
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, Some(&p1), 1102.3).expect("tl");
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, Some(&p1), 1105.0).expect("tl");
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
        let bar = DvdBarState::build(&p1, 100.0).expect("bar");
        assert_eq!(bar.tl.vobs.len(), 1);
        assert!(bar.chapter_preview_labels().is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd4_mounted_ch3_eof_advances_to_ch4() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD4/Video_ts/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let p1 = vob.to_path_buf();
        let p2 = vob.with_file_name("VTS_02_2.VOB");
        let p3 = vob.with_file_name("VTS_02_3.VOB");
        let p4 = vob.with_file_name("VTS_02_4.VOB");
        let bar = DvdBarState::build(&p1, 1102.0).expect("bar");
        let (next, loc, hold) = bar
            .tl
            .continue_after_vob_eof(&p3, 1097.0)
            .expect("ch3 eof continue");
        assert!(crate::video_ext::paths_same_file(&next, &p4));
        assert!(loc < 5.0, "local={loc}");
        assert!((hold - 3302.0).abs() < 5.0, "hold={hold}");
        let mut map = std::collections::HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1102.0);
        map.insert(p2.to_string_lossy().into_owned(), 1103.0);
        map.insert(p3.to_string_lossy().into_owned(), 1098.0);
        map.insert(p4.to_string_lossy().into_owned(), 924.0);
        let still =
            crate::dvd_entity::still_target_from_global(&p1, 3307.55, &map).expect("3307 resume");
        assert!(
            crate::video_ext::paths_same_file(&still.load, &p4),
            "3307 resume should open ch4, got {}",
            still.load.display()
        );
        assert!(still.local_sec < 30.0, "local={}", still.local_sec);
        let tl = crate::dvd_entity::build_title_timeline(&p1, &map, 1102.0).expect("tl");
        let p4_path = p4.clone();
        let (idx, local) = tl.resolve_global(3307.55);
        assert!(
            crate::video_ext::paths_same_file(tl.path_at(idx).expect("idx"), &p4_path),
            "3307 resume should map to ch4, got idx={idx} local={local:.2}"
        );
        assert!(local < 10.0, "local={local}");
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd4_multi_ifo_preview_caps_at_chapter_marks() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD4/Video_ts/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let p1 = vob.to_path_buf();
        let p4 = vob.with_file_name("VTS_02_4.VOB");
        let p5 = vob.with_file_name("VTS_03_1.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1102.0);
        map.insert(
            vob.with_file_name("VTS_02_2.VOB")
                .to_string_lossy()
                .into_owned(),
            1103.0,
        );
        map.insert(
            vob.with_file_name("VTS_02_3.VOB")
                .to_string_lossy()
                .into_owned(),
            1098.0,
        );
        map.insert(p4.to_string_lossy().into_owned(), 924.0);
        map.insert(p5.to_string_lossy().into_owned(), 1100.0);
        let tl = crate::dvd_entity::build_title_timeline(&p1, &map, 1102.0).expect("tl");
        let labels = chapter_labels_for_timeline(&tl);
        assert!(
            labels.len() >= 3,
            "expected IFO chapter marks across VTS sets, got {}",
            labels.len()
        );
        let vts03_start = tl.global_pos(&p5, 0.0);
        assert!(
            labels.iter().any(|(t, _)| (*t - vts03_start).abs() < 1.0),
            "VTS_03 block should add a chapter mark at {vts03_start}"
        );
        let bar = DvdBarState {
            tl,
            chapter_labels: labels,
        };
        let g = bar.global_pos(&p4, 900.0);
        let (idx, local) = bar.resolve_global(g);
        let dur = preview_chapter_dur(&bar, g, idx, local, &p4, &map);
        let remain_to_vts03 = vts03_start - g;
        assert!(
            dur <= local + remain_to_vts03 + 1.0,
            "preview cap should stop at next chapter (dur={dur}, remain={remain_to_vts03})"
        );
    }
}
