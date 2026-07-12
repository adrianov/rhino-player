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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
        assert!((tl.total_sec - 150.0).abs() < 1e-6);
        assert!((tl.global_pos(&p2, 10.0) - 110.0).abs() < 1e-6);
        let (idx, local) = tl.resolve_global(110.0);
        assert_eq!(idx, 1);
        assert!((local - 10.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn title_chapter_paths_scoped_to_one_title_set() {
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
        let list = crate::dvd_entity::title_chapter_paths(&p21).expect("paths");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|p| crate::dvd_entity::vob_title_id(p) == Some(2)));
        let all = crate::dvd_entity::timeline_chapter_paths(&p21).expect("timeline");
        assert_eq!(all.len(), 4);
        assert!(
            all.iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB"))
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn timeline_spans_feature_sets_with_per_vob_durs() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-tl-sets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_02_1.VOB");
        write_vob(&vts, "VTS_02_2.VOB");
        write_vob(&vts, "VTS_03_1.VOB");
        write_vob(&vts, "VTS_03_2.VOB");
        let p21 = vts.join("VTS_02_1.VOB");
        let p31 = vts.join("VTS_03_1.VOB");
        let mut map = std::collections::HashMap::new();
        map.insert(p21.to_string_lossy().into_owned(), 100.0);
        map.insert(
            vts.join("VTS_02_2.VOB").to_string_lossy().into_owned(),
            50.0,
        );
        map.insert(p31.to_string_lossy().into_owned(), 200.0);
        map.insert(
            vts.join("VTS_03_2.VOB").to_string_lossy().into_owned(),
            80.0,
        );
        let tl = DvdVobTimeline::from_title_vobs(&p21, &map, 100.0).expect("tl");
        assert!((tl.total_sec - 430.0).abs() < 1e-6);
        let (idx, local) = tl.resolve_global(150.0);
        assert_eq!(idx, 2);
        assert!((local - 0.0).abs() < 1e-6);
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
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
        let still = crate::dvd_entity::still_at_global(&p1, 350.0, &map, None, None).expect("still");
        assert!(crate::video_ext::paths_same_file(&still.load, &p3));
        assert!((still.local_sec - 50.0).abs() < 1.0, "local={}", still.local_sec);
        let still2 = crate::dvd_entity::still_at_global(&base, 350.0, &map, None, None).expect("disc");
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
        let still = crate::dvd_entity::still_at_global(&p1, 1400.0, &map, None, None).expect("still");
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &HashMap::new(), 0.0);
        assert!(tl.is_none(), "invalid .vob bytes must not invent a timeline");
        let _ = fs::remove_dir_all(&base);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn mgnoveniy_dvd3_disc_timeline_when_mounted() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD3/VIDEO_TS/VTS_02_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let paths = crate::dvd_entity::timeline_chapter_paths(vob).expect("paths");
        assert_eq!(paths.len(), 8, "VTS_02 (4) + VTS_03 (4) chapter files");
        let durs = crate::dvd_ifo_parse::title_vob_durations(vob).expect("ifo durs");
        assert_eq!(durs.len(), 8);
        let total: f64 = durs.iter().sum();
        assert!(
            total > 8200.0 && total < 8300.0,
            "disc feature bar should be ~2h 18m, got {total:.1}s"
        );
        let tl = DvdVobTimeline::from_title_vobs_with(
            vob,
            &std::collections::HashMap::new(),
            0.0,
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .expect("timeline");
        assert!(
            (tl.total_sec - total).abs() < 1.0,
            "bar total {:.1} vs ifo {total:.1}",
            tl.total_sec
        );
        let (idx, _) = tl.resolve_global(4000.0);
        assert_eq!(
            tl.vobs[idx].file_name().and_then(|n| n.to_str()),
            Some("VTS_03_1.VOB"),
            "mid-disc global time should land in VTS_03"
        );
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
        let list = crate::dvd_entity::timeline_chapter_paths(vob).expect("disc chapters");
        assert!(
            list.len() >= 4,
            "DVD5 should have several chapter files, got {}",
            list.len()
        );
        crate::dvd_vob_mpv_probe::clear_probe_cache();
        let disc = crate::video_ext::dvd_disc_root(vob).expect("disc");
        let mut map = HashMap::new();
        map.insert(disc.to_string_lossy().into_owned(), 1131.1);
        let tl = DvdVobTimeline::from_title_vobs_with(
            vob,
            &map,
            1129.0,
            crate::dvd_entity::TimelineBuildOpts::FULL,
        )
        .expect("tl");
        assert!(
            tl.missing_dur_count() <= 1,
            "at most one .vob may fail headless probe, missing={}",
            tl.missing_dur_count()
        );
        assert!(
            tl.vobs.len() == list.len(),
            "timeline should cover all disc feature chapter files"
        );
        assert!(
            tl.total_sec > 1000.0,
            "disc feature bar should span chapter files, got {:.1}s",
            tl.total_sec
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1102.3).expect("tl");
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
    fn chain_head_eof_continue_uses_ifo_local_not_mpv_tail() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-ch-eof-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_01_2.VOB");
        write_vob(&vts, "VTS_01_6.VOB");
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let p6 = vts.join("VTS_01_6.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1062.12);
        map.insert(p2.to_string_lossy().into_owned(), 1069.92);
        map.insert(p6.to_string_lossy().into_owned(), 487.6);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 90_658.28).expect("tl");
        let mpv_dur = 90_658.28;
        let mpv_pos_mid = chain_head_ifo_local_to_mpv(1056.09, mpv_dur, 1062.12, true);
        let ifo_mid = timeline_local_from_mpv(&tl, &p1, mpv_pos_mid, mpv_dur);
        assert!((ifo_mid - 1056.09).abs() < 0.1, "ifo mid={ifo_mid}");
        assert!(
            (1062.12 - ifo_mid) > crate::app::TICK_EOF_TAIL_SEC,
            "virtual tail mid-chapter must not look like EOF"
        );
        let (next_bad, _, g_bad) = tl
            .continue_after_vob_eof(&p1, mpv_pos_mid)
            .expect("wrong mpv tail maps to title end");
        assert!(
            crate::video_ext::paths_same_file(&next_bad, &p6),
            "raw mpv tail skips to last vob, got {}",
            next_bad.display()
        );
        assert!((g_bad - tl.total_sec).abs() < 0.2, "g_bad={g_bad}");
        let (next_ok, local_ok, g_ok) = tl
            .continue_after_vob_eof(&p1, ifo_mid.max(1062.12 - 0.05))
            .expect("ifo tail advances to next chapter");
        assert!(
            crate::video_ext::paths_same_file(&next_ok, &p2),
            "ifo tail advances to vob2, got {}",
            next_ok.display()
        );
        assert!(local_ok < 1.0, "local_ok={local_ok}");
        assert!((g_ok - 1062.05).abs() < 0.2, "g_ok={g_ok}");
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
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1105.0).expect("tl");
        assert!(
            tl.next_chapter_after(&p1).is_some(),
            "second chapter must remain reachable when total equals first chapter only"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn bar_cache_not_stale_for_full_title_on_short_chapter() {
        // Not `rhino-dvd-stale`: `dvd_entity` tests use that dir and run in parallel.
        let base = std::env::temp_dir().join(format!("rhino-dvd-bar-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        for n in 1..=5 {
            write_vob(&vts, &format!("VTS_02_{n}.VOB"));
        }
        let p5 = vts.join("VTS_02_5.VOB");
        let mut map = HashMap::new();
        for n in 1..=5 {
            let p = vts.join(format!("VTS_02_{n}.VOB"));
            map.insert(p.to_string_lossy().into_owned(), 1000.0);
        }
        let bar = DvdBarState::build_with_map(&p5, 207.0, &map).expect("bar");
        assert_eq!(bar.tl.vobs.len(), 5);
        assert!(bar.total_sec() > 207.0 * 5.0 * 1.5);
        assert!(!bar_cache_stale(&bar, 207.0, 5, Some(&p5)));
        let p1 = vts.join("VTS_02_1.VOB");
        let mut map_one = HashMap::new();
        map_one.insert(p1.to_string_lossy().into_owned(), 1000.0);
        let bar_one = DvdBarState::build_with_map(&p1, 1000.0, &map_one).expect("bar one");
        assert!(bar_cache_stale(&bar_one, 1000.0, 5, Some(&p1)));
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
        let map = crate::db::load_duration_map();
        let bar = DvdBarState::build_with_map_opts(
            &p1,
            1102.0,
            &map,
            crate::dvd_entity::TimelineBuildOpts::FULL,
        )
        .expect("bar");
        let i3 = bar.tl.index_of(&p3).expect("p3 idx");
        let i4 = bar.tl.index_of(&p4).expect("p4 idx");
        let (next, loc, hold) = bar
            .tl
            .continue_after_vob_eof(&p3, 1097.0)
            .expect("ch3 eof continue");
        assert!(crate::video_ext::paths_same_file(&next, &p4));
        assert!(loc < 5.0, "local={loc}");
        let expected_hold = (bar.tl.starts[i3] + 1097.0 + 0.05).min(bar.tl.total_sec);
        assert!(
            (hold - expected_hold).abs() < 0.1,
            "hold={hold} expected={expected_hold}"
        );
        let mut map = std::collections::HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1102.0);
        map.insert(p2.to_string_lossy().into_owned(), 1103.0);
        map.insert(p3.to_string_lossy().into_owned(), 1098.0);
        map.insert(p4.to_string_lossy().into_owned(), 924.0);
        let resume_g = bar.tl.starts[i4] + 5.0;
        let still =
            crate::dvd_entity::still_at_global(&p1, resume_g, &map, None, None).expect("ch4 resume");
        assert!(
            crate::video_ext::paths_same_file(&still.load, &p4),
            "resume should open ch4, got {}",
            still.load.display()
        );
        assert!(still.local_sec < 10.0, "local={}", still.local_sec);
        let tl = crate::dvd_entity::build_title_timeline(&p1, &map, 1102.0).expect("tl");
        let p4_path = p4.clone();
        let (idx, local) = tl.resolve_global(resume_g);
        assert!(
            crate::video_ext::paths_same_file(tl.path_at(idx).expect("idx"), &p4_path),
            "resume should map to ch4, got idx={idx} local={local:.2}"
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
        let tl = crate::dvd_entity::build_title_timeline(&p1, &map, 1102.0).expect("tl");
        let labels = chapter_labels_for_timeline(&tl);
        assert!(
            labels.len() >= 2,
            "expected IFO chapter marks within VTS_02, got {}",
            labels.len()
        );
        let bar = DvdBarState {
            tl,
            chapter_labels: labels,
        };
        let g = bar.global_pos(&p4, 900.0);
        let (idx, local) = bar.resolve_global(g);
        let dur = preview_chapter_dur(&bar, g, idx, local, &p4, &map);
        assert!(
            dur <= local + 30.0,
            "preview cap should stay near chapter end (dur={dur}, local={local})"
        );
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd9_ifo_timeline_rejects_bogus_mpv_duration() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let bogus = 90_658.0;
        assert_eq!(crate::dvd_vob_timeline::clamp_vob_duration(bogus), 0.0);
        let tl = DvdVobTimeline::from_title_vobs_with(
            vob,
            &std::collections::HashMap::new(),
            bogus,
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .expect("timeline");
        assert!(
            (tl.total_sec - 5842.0).abs() < 5.0,
            "IFO sector bar total should be ~97 min, got {:.1}s",
            tl.total_sec
        );
        let first = tl.vobs.first().expect("vobs");
        assert_eq!(
            first.file_name().and_then(|n| n.to_str()),
            Some("VTS_01_1.VOB"),
            "full-size VTS_01_1 stays in unified timeline for splash"
        );
        assert!(
            tl.durs[0] > 1050.0 && tl.durs[0] < 1080.0,
            "first segment from IFO sectors, got {}",
            tl.durs[0]
        );
        let tl_mpv = DvdVobTimeline::from_title_vobs_with(
            vob,
            &std::collections::HashMap::new(),
            90_658.0,
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .expect("timeline with bogus mpv dur");
        assert!(
            (tl.total_sec - tl_mpv.total_sec).abs() < 1.0,
            "IFO total must not drift with mpv live dur ({} vs {})",
            tl.total_sec,
            tl_mpv.total_sec
        );
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn fritt_resume_chapter6_seek_to_start() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_6.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let bar = DvdBarState::build_with_map(vob, 1072.0, &HashMap::new()).expect("bar");
        assert!((bar.total_sec() - 5842.0).abs() < 5.0, "bar total {:.1}s", bar.total_sec());
        assert!(
            bar.chapter_dur_at(0) > 1050.0 && bar.chapter_dur_at(0) < 1080.0,
            "VTS_01_1 ~1062s, got {:.1}s",
            bar.chapter_dur_at(0)
        );
        let (idx, local) = bar.resolve_global(0.0);
        assert_eq!(idx, 0);
        assert!(local.abs() < 1e-6);
        assert_eq!(
            bar.path_at(idx).and_then(|p| p.file_name().and_then(|n| n.to_str())),
            Some("VTS_01_1.VOB")
        );
        assert_fritt_preview_dur(&bar, 0.0, bar.chapter_dur_at(0) * 0.95);
        assert_fritt_preview_dur(&bar, 500.0, 900.0);
        let g = bar.global_pos(vob, 0.0);
        let (idx6, local6) = bar.resolve_global(g);
        assert_eq!(idx6, bar.tl.index_of(vob).expect("idx"));
        assert!(local6.abs() < 1.0);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn fritt_chain_head_implausible_mpv_pos() {
        let vob = std::path::Path::new(
            "/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        assert!(crate::dvd_vob_mpv_probe::is_title_chain_head(vob));
        let bar = DvdBarState::build_with_map(vob, 0.0, &HashMap::new()).expect("bar");
        let seg = bar.chapter_dur_at(0);
        assert!(seg > 1050.0 && seg < 1080.0);
        let (idx, local) = bar.resolve_global(500.0);
        assert_eq!(idx, 0);
        assert!((local - 500.0).abs() < 1.0);
        assert!(!bar.tl.ifo_segment_local_plausible(vob, 5654.0));
        assert!((bar.tl.clamp_ifo_segment_local(vob, 5654.0) - seg).abs() < 0.1);
    }

    #[test]
    fn chain_ifo_local_to_mpv_tail() {
        let seg = 1062.0;
        let dur = 90_658.0;
        let tail = dur - seg;
        assert!(
            (chain_head_ifo_local_to_mpv(520.0, dur, seg, true) - (tail + 520.0)).abs() < 1e-6
        );
        assert!((chain_head_ifo_local_to_mpv(520.0, dur, seg, false) - 520.0).abs() < 1e-6);
        assert!((chain_head_ifo_local_to_mpv(1062.0, dur, seg, true) - dur).abs() < 0.1);
    }

    #[test]
    fn chain_ifo_local_from_mpv_tail() {
        let seg = 1062.0;
        let dur = 90_658.0;
        let tail = dur - seg;
        assert!(
            (super::chain_head_ifo_local_from_mpv(520.0, dur, seg) - 520.0).abs() < 1e-6
        );
        assert!(
            (super::chain_head_ifo_local_from_mpv(tail + 520.0, dur, seg) - 520.0).abs() < 1e-6
        );
        assert!(super::chain_head_ifo_local_from_mpv(tail, dur, seg).abs() < 1e-6);
    }

    #[test]
    fn chain_bar_sync_ifo_local() {
        use crate::dvd_vob_timeline::DvdChainBarSync;
        let sync = DvdChainBarSync {
            anchor_local: 520.0,
            anchor_global: 520.0,
            anchor_playback: 100.0,
        };
        assert!((sync.global_from_ifo_local(520.0, 100.0, 6000.0) - 520.0).abs() < 1e-6);
        assert!((sync.global_from_ifo_local(520.0, 105.0, 6000.0) - 525.0).abs() < 1e-6);
        assert!((sync.global_from_ifo_local(530.0, 100.0, 6000.0) - 530.0).abs() < 1e-6);
    }

    #[test]
    fn chain_mpv_seek_always_tail_when_stretched() {
        let seg = 1062.0;
        let dur = 90_658.0;
        let tail = dur - seg;
        assert!(
            (chain_head_ifo_local_to_mpv(419.0, dur, seg, true) - (tail + 419.0)).abs() < 1e-6
        );
        assert!(
            (chain_head_ifo_local_to_mpv(419.0, dur, seg, false) - 419.0).abs() < 1e-6
        );
    }

    #[test]
    fn chain_head_mpv_ready_stretched() {
        let seg = 1062.0;
        assert!(!chain_head_stretched(1062.0, seg));
        assert!(chain_head_stretched(90_658.0, seg));
    }

    #[test]
    fn preview_mpv_seek_tail_when_stretched() {
        let seg = 1062.0;
        let dur = 90_658.0;
        let ifo = 419.0;
        let mpv_t = chain_head_ifo_local_to_mpv(ifo, dur, seg, true);
        assert!((mpv_t - (dur - seg + ifo)).abs() < 1e-6);
    }

    #[test]
    fn timeline_local_from_mpv_chain_tail() {
        let base = std::env::temp_dir().join(format!("rhino-dvd-persist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        write_vob(&vts, "VTS_01_1.VOB");
        write_vob(&vts, "VTS_01_2.VOB");
        let p1 = vts.join("VTS_01_1.VOB");
        let mut map = HashMap::new();
        map.insert(p1.to_string_lossy().into_owned(), 1062.0);
        map.insert(
            vts.join("VTS_01_2.VOB").to_string_lossy().into_owned(),
            1069.0,
        );
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1062.0).expect("tl");
        let mpv_dur = 90_658.0;
        let mpv_pos = mpv_dur - 1062.0 + 125.73;
        let local = timeline_local_from_mpv(&tl, &p1, mpv_pos, mpv_dur);
        assert!(
            (local - 125.73).abs() < 0.1,
            "ifo local from virtual tail, got {local}"
        );
        let global = tl.global_pos(&p1, local);
        assert!(
            (global - 125.73).abs() < 0.1,
            "global must not clamp to title end, got {global}"
        );
        let snap = crate::dvd_entity::playback_snapshot(&p1, mpv_pos, mpv_dur, &map).expect("snap");
        assert!(
            (snap.1 - 125.73).abs() < 0.1,
            "persist snapshot global, got {}",
            snap.1
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn chain_bar_sync_from_targets() {
        use crate::dvd_vob_timeline::DvdChainBarSync;
        let sync = DvdChainBarSync::from_targets(419.0, 419.0, 89597.0);
        assert!((sync.anchor_local - 419.0).abs() < 1e-6);
        assert!((sync.global_from_ifo_local(419.0, 89597.0, 6000.0) - 419.0).abs() < 1e-6);
        assert!((sync.global_from_ifo_local(930.0, 89600.0, 6000.0) - 930.0).abs() < 1e-6);
    }

    fn assert_fritt_preview_dur(bar: &DvdBarState, global: f64, min_dur: f64) {
        let (idx, local) = bar.resolve_global(global);
        let load = bar.path_at(idx).expect("load");
        let dur = preview_chapter_dur(bar, global, idx, local, load, &HashMap::new());
        assert!(
            dur >= min_dur,
            "global={global:.1} preview dur={dur:.1} (min {min_dur:.1})"
        );
    }
}
