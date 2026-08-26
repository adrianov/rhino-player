#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn write_vob(dir: &std::path::Path, name: &str) {
        fs::write(dir.join(name), b"vob").expect("write");
    }

    /// Fresh `VIDEO_TS` sandbox with a dummy `VIDEO_TS.IFO`; caller removes `base` when done.
    fn temp_vts(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!("rhino-dvd-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let vts = base.join("VIDEO_TS");
        fs::create_dir_all(&vts).expect("mkdir");
        fs::write(vts.join("VIDEO_TS.IFO"), b"DVD").expect("ifo");
        (base, vts)
    }

    fn rm_rf(path: &std::path::Path) {
        let _ = fs::remove_dir_all(path);
    }

    fn write_vobs(vts: &std::path::Path, names: &[&str]) {
        for name in names {
            write_vob(vts, name);
        }
    }

    /// Duration map keyed by lossy path strings.
    fn dur_map(pairs: &[(&std::path::Path, f64)]) -> HashMap<String, f64> {
        pairs
            .iter()
            .map(|(p, d)| (p.to_string_lossy().into_owned(), *d))
            .collect()
    }

    fn put_dur(map: &mut HashMap<String, f64>, p: &std::path::Path, dur: f64) {
        map.insert(p.to_string_lossy().into_owned(), dur);
    }

    /// Sequential `VTS_02_N.VOB` durations inserted into `map`.
    fn put_sequential_durs(vts: &std::path::Path, map: &mut HashMap<String, f64>, durs: &[f64]) {
        for (i, dur) in durs.iter().enumerate() {
            put_dur(map, &vts.join(format!("VTS_02_{}.VOB", i + 1)), *dur);
        }
    }

    /// Sample-disc `.vob` path when the rip is mounted; `None` skips the test.
    fn mounted_vob(path: &str) -> Option<std::path::PathBuf> {
        let vob = std::path::PathBuf::from(path);
        vob.is_file().then_some(vob)
    }

    fn assert_close(actual: f64, want: f64) {
        assert!((actual - want).abs() < 1e-6, "got {actual}, want {want}");
    }

    fn assert_resolve_is(tl: &DvdVobTimeline, global: f64, want_idx: usize, want_local: f64) {
        let (idx, local) = tl.resolve_global(global);
        assert_eq!(idx, want_idx);
        assert_close(local, want_local);
    }

    fn assert_resolve_index(tl: &DvdVobTimeline, global: f64, want_idx: usize) {
        assert_eq!(tl.resolve_global(global).0, want_idx);
    }

    /// `still_at_global` with no bar/cap must land on `want_load` near `want_local`.
    fn assert_still_at_global_maps(
        map: &HashMap<String, f64>,
        from: &std::path::Path,
        global: f64,
        want_load: &std::path::Path,
        want_local: f64,
        tol: f64,
    ) {
        let still =
            crate::dvd_entity::still_at_global(from, global, map, None, None).expect("still");
        assert!(
            crate::video_ext::paths_same_file(&still.load, want_load),
            "expected {} got {}",
            want_load.display(),
            still.load.display()
        );
        assert!(
            (still.local_sec - want_local).abs() < tol,
            "local={}",
            still.local_sec
        );
    }

    fn eof_continue(
        tl: &DvdVobTimeline,
        from: &std::path::Path,
        local_pos: f64,
        label: &str,
    ) -> (std::path::PathBuf, f64, f64) {
        tl.continue_after_vob_eof(from, local_pos).expect(label)
    }

    /// EOF continue must advance to `want` near its start while holding `want_g`.
    fn assert_eof_advances(
        tl: &DvdVobTimeline,
        from: &std::path::Path,
        local_pos: f64,
        want: &std::path::Path,
        want_local: f64,
        want_g: f64,
        label: &str,
    ) {
        let (next, local, g) = eof_continue(tl, from, local_pos, label);
        assert!(
            crate::video_ext::paths_same_file(&next, want),
            "{label}: expected {} got {}",
            want.display(),
            next.display()
        );
        assert!(
            (local - want_local).abs() < 0.1,
            "{label}: local spill {local}"
        );
        assert!((g - want_g).abs() < 0.1, "{label}: hold global {g}");
    }

    /// CACHE_ONLY timeline whose bar total matches the summed IFO durations.
    fn cache_only_timeline_matching_ifo(vob: &std::path::Path, ifo_total: f64) -> DvdVobTimeline {
        let tl = DvdVobTimeline::from_title_vobs_with(
            vob,
            &std::collections::HashMap::new(),
            0.0,
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .expect("timeline");
        assert!(
            (tl.total_sec - ifo_total).abs() < 1.0,
            "bar total {:.1} vs ifo {ifo_total:.1}",
            tl.total_sec
        );
        tl
    }

    #[test]
    fn global_pos_and_resolve_round_trip() {
        let (base, vts) = temp_vts("tl");
        write_vobs(&vts, &["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let map = dur_map(&[(&p1, 100.0), (&p2, 50.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
        assert_close(tl.total_sec, 150.0);
        assert_close(tl.global_pos(&p2, 10.0), 110.0);
        assert_resolve_is(&tl, 110.0, 1, 10.0);
        rm_rf(&base);
    }

    /// Timeline paths cover both feature sets including the later title's first chapter.
    fn assert_timeline_spans_feature_sets(all: &[std::path::PathBuf]) {
        assert_eq!(all.len(), 4);
        assert!(all
            .iter()
            .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB")));
    }

    #[test]
    fn title_chapter_paths_scoped_to_one_title_set() {
        let (base, vts) = temp_vts("feat");
        write_vobs(
            &vts,
            &[
                "VTS_01_1.VOB",
                "VTS_02_1.VOB",
                "VTS_02_2.VOB",
                "VTS_03_1.VOB",
                "VTS_03_2.VOB",
            ],
        );
        let p21 = vts.join("VTS_02_1.VOB");
        let list = crate::dvd_entity::title_chapter_paths(&p21).expect("paths");
        assert_eq!(list.len(), 2);
        assert!(list
            .iter()
            .all(|p| crate::dvd_entity::vob_title_id(p) == Some(2)));
        let all = crate::dvd_entity::timeline_chapter_paths(&p21).expect("timeline");
        assert_timeline_spans_feature_sets(&all);
        rm_rf(&base);
    }

    #[test]
    fn timeline_spans_feature_sets_with_per_vob_durs() {
        let (base, vts) = temp_vts("tl-sets");
        write_vobs(
            &vts,
            &[
                "VTS_02_1.VOB",
                "VTS_02_2.VOB",
                "VTS_03_1.VOB",
                "VTS_03_2.VOB",
            ],
        );
        let p21 = vts.join("VTS_02_1.VOB");
        let p31 = vts.join("VTS_03_1.VOB");
        let map = dur_map(&[
            (&p21, 100.0),
            (&vts.join("VTS_02_2.VOB"), 50.0),
            (&p31, 200.0),
            (&vts.join("VTS_03_2.VOB"), 80.0),
        ]);
        let tl = DvdVobTimeline::from_title_vobs(&p21, &map, 100.0).expect("tl");
        assert_close(tl.total_sec, 430.0);
        assert_resolve_is(&tl, 150.0, 2, 0.0);
        rm_rf(&base);
    }

    #[test]
    fn multi_chapter_total_is_sum_of_vob_lengths() {
        let (base, vts) = temp_vts("tl-total");
        write_vobs(&vts, &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let map = dur_map(&[(&p1, 100.0), (&p2, 50.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
        assert_close(tl.total_sec, 150.0);
        rm_rf(&base);
    }

    #[test]
    fn resume_maps_global_with_per_vob_durations() {
        let (base, vts) = temp_vts("ent-res");
        for (i, n) in [100usize, 200, 300, 400].iter().enumerate() {
            fs::write(vts.join(format!("VTS_02_{}.VOB", i + 1)), vec![b'x'; *n]).expect("vob");
        }
        let mut map = HashMap::new();
        put_sequential_durs(&vts, &mut map, &[100.0, 200.0, 300.0, 400.0]);
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        assert_still_at_global_maps(&map, &p1, 350.0, &p3, 50.0, 1.0);
        assert_still_at_global_maps(&map, &base, 350.0, &p3, 50.0, 1.0);
        rm_rf(&base);
    }

    #[test]
    fn map_durations_resolve_global() {
        let (base, vts) = temp_vts("fill");
        for kb in 1..=4usize {
            fs::write(vts.join(format!("VTS_02_{kb}.VOB")), vec![0u8; kb * 1000]).expect("write");
        }
        let p1 = vts.join("VTS_02_1.VOB");
        let p3 = vts.join("VTS_02_3.VOB");
        let mut map = dur_map(&[(&crate::playback_entity::db_path_for(&p1), 4000.0)]);
        put_sequential_durs(&vts, &mut map, &[600.0, 500.0, 400.0, 400.0]);
        assert_still_at_global_maps(&map, &p1, 1400.0, &p3, 300.0, 2.0);
        rm_rf(&base);
    }

    #[test]
    fn preview_chapter_dur_caps_at_next_mark() {
        let (base, vts) = temp_vts("prev-cap");
        write_vobs(&vts, &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
        let p1 = vts.join("VTS_02_1.VOB");
        let map = dur_map(&[(&p1, 100.0), (&vts.join("VTS_02_2.VOB"), 50.0)]);
        let bar = DvdBarState::build(&p1, 100.0).expect("bar");
        let dur = preview_chapter_dur(&bar, 90.0, 0, 90.0, &p1, &map);
        assert_close(dur, 100.0);
        rm_rf(&base);
    }

    #[test]
    fn ifo_chapter_labels_scale_to_vob_total() {
        let ifo = crate::dvd_ifo_parse::IfoChapterMarks {
            mark_secs: vec![1000.0, 2000.0],
            title_sec: 4000.0,
        };
        let scale = 4200.0 / ifo.title_sec;
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
        let (base, vts) = temp_vts("noanchor");
        fs::write(vts.join("VTS_02_1.VOB"), vec![0u8; 1000]).expect("write");
        fs::write(vts.join("VTS_02_2.VOB"), vec![0u8; 2000]).expect("write");
        let p1 = vts.join("VTS_02_1.VOB");
        let tl = DvdVobTimeline::from_title_vobs(&p1, &HashMap::new(), 0.0);
        assert!(
            tl.is_none(),
            "invalid .vob bytes must not invent a timeline"
        );
        rm_rf(&base);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn mgnoveniy_dvd3_disc_timeline_when_mounted() {
        let Some(vob) = mounted_vob(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD3/VIDEO_TS/VTS_02_1.VOB",
        ) else {
            return;
        };
        let paths = crate::dvd_entity::timeline_chapter_paths(&vob).expect("paths");
        assert_eq!(paths.len(), 8, "VTS_02 (4) + VTS_03 (4) chapter files");
        let durs = crate::dvd_ifo_parse::title_vob_durations(&vob).expect("ifo durs");
        assert_eq!(durs.len(), 8);
        let total: f64 = durs.iter().sum();
        assert!(
            total > 8200.0 && total < 8300.0,
            "disc feature bar should be ~2h 18m, got {total:.1}s"
        );
        let tl = cache_only_timeline_matching_ifo(&vob, total);
        assert_eq!(
            tl.vobs[tl.resolve_global(4000.0).0]
                .file_name()
                .and_then(|n| n.to_str()),
            Some("VTS_03_1.VOB"),
            "mid-disc global time should land in VTS_03"
        );
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd5_mpv_probe_fills_full_timeline() {
        let Some(vob) = mounted_vob(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD5/VIDEO_TS/VTS_02_1.VOB",
        ) else {
            return;
        };
        let list = crate::dvd_entity::timeline_chapter_paths(&vob).expect("disc chapters");
        assert!(
            list.len() >= 4,
            "DVD5 should have several chapter files, got {}",
            list.len()
        );
        crate::dvd_vob_mpv_probe::clear_probe_cache();
        let map = dur_map(&[(
            &crate::video_ext::dvd_disc_root(&vob).expect("disc"),
            1131.1,
        )]);
        let tl = DvdVobTimeline::from_title_vobs_with(
            &vob,
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
        assert_eq!(
            tl.vobs.len(),
            list.len(),
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
        assert!(list
            .iter()
            .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("VTS_03_1.VOB")));
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
            assert_eq!(labels.first().map(|(_, s)| s.as_str()), Some("Chapter 1"));
        }
    }

    #[test]
    fn resolve_picks_chapter_by_duration_window() {
        let (base, vts) = temp_vts("win");
        write_vobs(&vts, &["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let map = dur_map(&[(&p1, 100.0), (&p2, 40.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 100.0).expect("tl");
        assert_resolve_is(&tl, 105.0, 1, 5.0);
        assert_resolve_index(&tl, 10.0, 0);
        rm_rf(&base);
    }

    #[test]
    fn eof_continue_uses_live_tail_beyond_stored_chapter_dur() {
        let (base, vts) = temp_vts("eof-tail");
        write_vobs(&vts, &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
        let p1 = vts.join("VTS_02_1.VOB");
        let p2 = vts.join("VTS_02_2.VOB");
        let map = dur_map(&[(&p1, 1102.3), (&p2, 1100.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1102.3).expect("tl");
        assert_eof_advances(&tl, &p1, 1104.78, &p2, 2.53, 1104.83, "continue");
        let (_, local0, _) = eof_continue(&tl, &p1, 1102.3, "at stored end");
        assert!(local0 < 0.1, "stored end lands at next vob start");
        rm_rf(&base);
    }

    /// Raw stretched mpv tails must not drive EOF; IFO-local positions must.
    fn assert_chain_head_eof_prefers_ifo_local(vts: &std::path::Path) {
        let p1 = vts.join("VTS_01_1.VOB");
        let p2 = vts.join("VTS_01_2.VOB");
        let p6 = vts.join("VTS_01_6.VOB");
        let map = dur_map(&[(&p1, 1062.12), (&p2, 1069.92), (&p6, 487.6)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 90_658.28).expect("tl");
        let mpv_dur = 90_658.28;
        let seg = 1062.12;
        let mpv_pos_mid = chain_head_ifo_local_to_mpv(1056.09, mpv_dur, seg, true);
        assert_round_trip_ifo_local(&tl, &p1, mpv_pos_mid, mpv_dur, 1056.09);
        assert!(
            (seg - 1056.09) > crate::app::TICK_EOF_TAIL_SEC,
            "virtual tail mid-chapter must not look like EOF"
        );
        assert_raw_mpv_tail_skips_to_last(&tl, &p1, &p6, mpv_pos_mid);
        assert_ifo_tail_advances_to_next(&tl, &p1, &p2, 1056.09f64.max(seg - 0.05));
    }

    /// Stretched-tail mapping brings an IFO-local position back unchanged.
    fn assert_round_trip_ifo_local(
        tl: &DvdVobTimeline,
        p1: &std::path::Path,
        mpv_pos: f64,
        mpv_dur: f64,
        want_ifo_local: f64,
    ) {
        let ifo_mid = timeline_local_from_mpv(tl, p1, mpv_pos, mpv_dur);
        assert!((ifo_mid - want_ifo_local).abs() < 0.1, "ifo mid={ifo_mid}");
    }

    fn assert_raw_mpv_tail_skips_to_last(
        tl: &DvdVobTimeline,
        p1: &std::path::Path,
        last: &std::path::Path,
        mpv_pos: f64,
    ) {
        let (next_bad, _, g_bad) =
            eof_continue(tl, p1, mpv_pos, "wrong mpv tail maps to title end");
        assert!(
            crate::video_ext::paths_same_file(&next_bad, last),
            "raw mpv tail skips to last vob, got {}",
            next_bad.display()
        );
        assert!((g_bad - tl.total_sec).abs() < 0.2, "g_bad={g_bad}");
    }

    fn assert_ifo_tail_advances_to_next(
        tl: &DvdVobTimeline,
        p1: &std::path::Path,
        next: &std::path::Path,
        pos: f64,
    ) {
        let (next_ok, local_ok, g_ok) =
            eof_continue(tl, p1, pos, "ifo tail advances to next chapter");
        assert!(
            crate::video_ext::paths_same_file(&next_ok, next),
            "ifo tail advances to vob2, got {}",
            next_ok.display()
        );
        assert!(local_ok < 1.0, "local_ok={local_ok}");
        assert!((g_ok - 1062.05).abs() < 0.2, "g_ok={g_ok}");
    }

    #[test]
    fn chain_head_eof_continue_uses_ifo_local_not_mpv_tail() {
        let (base, vts) = temp_vts("ch-eof");
        write_vobs(&vts, &["VTS_01_1.VOB", "VTS_01_2.VOB", "VTS_01_6.VOB"]);
        assert_chain_head_eof_prefers_ifo_local(&vts);
        rm_rf(&base);
    }

    #[test]
    fn stale_total_still_lists_next_chapter() {
        let (base, vts) = temp_vts("eof");
        write_vobs(&vts, &["VTS_02_1.VOB", "VTS_02_2.VOB"]);
        let p1 = vts.join("VTS_02_1.VOB");
        let map = dur_map(&[(&p1, 1105.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1105.0).expect("tl");
        assert!(
            tl.next_chapter_after(&p1).is_some(),
            "second chapter must remain reachable when total equals first chapter only"
        );
        rm_rf(&base);
    }

    /// Bar built on the last chapter covers all five queued chapters.
    fn assert_full_five_chapter_bar_fresh(vts: &std::path::Path) {
        let p5 = vts.join("VTS_02_5.VOB");
        let mut map = HashMap::new();
        for n in 1..=5 {
            put_dur(&mut map, &vts.join(format!("VTS_02_{n}.VOB")), 1000.0);
        }
        let bar = DvdBarState::build_with_map(&p5, 207.0, &map).expect("bar");
        assert_eq!(bar.tl.vobs.len(), 5);
        assert!(bar.total_sec() > 207.0 * 5.0 * 1.5);
        assert!(!bar_cache_stale(&bar, 207.0, 5, Some(&p5)));
    }

    fn assert_short_chapter_bar_stale(vts: &std::path::Path) {
        let p1 = vts.join("VTS_02_1.VOB");
        let map = dur_map(&[(&p1, 1000.0)]);
        let bar_one = DvdBarState::build_with_map(&p1, 1000.0, &map).expect("bar one");
        assert!(bar_cache_stale(&bar_one, 1000.0, 5, Some(&p1)));
    }

    #[test]
    fn bar_cache_not_stale_for_full_title_on_short_chapter() {
        // Not `rhino-dvd-stale`: `dvd_entity` tests use that dir and run in parallel.
        let (base, vts) = temp_vts("bar-stale");
        for n in 1..=5 {
            write_vob(&vts, &format!("VTS_02_{n}.VOB"));
        }
        assert_full_five_chapter_bar_fresh(&vts);
        assert_short_chapter_bar_stale(&vts);
        rm_rf(&base);
    }

    #[test]
    fn preview_labels_empty_for_single_chapter_vob() {
        let (base, vts) = temp_vts("one-ch");
        write_vob(&vts, "VTS_02_1.VOB");
        let p1 = vts.join("VTS_02_1.VOB");
        let bar = DvdBarState::build(&p1, 100.0).expect("bar");
        assert_eq!(bar.tl.vobs.len(), 1);
        assert!(bar.chapter_preview_labels().is_empty());
        rm_rf(&base);
    }

    /// FULL-probe DVD4 bar over the real persisted duration map.
    fn dvd4_full_bar(vob: &std::path::Path) -> DvdBarState {
        let map = crate::db::load_duration_map();
        DvdBarState::build_with_map_opts(
            vob,
            1102.0,
            &map,
            crate::dvd_entity::TimelineBuildOpts::FULL,
        )
        .expect("bar")
    }

    fn assert_ch3_eof_continues_to_ch4(bar: &DvdBarState, vob: &std::path::Path) {
        let p3 = vob.with_file_name("VTS_02_3.VOB");
        let p4 = vob.with_file_name("VTS_02_4.VOB");
        let i3 = bar.tl.index_of(&p3).expect("p3 idx");
        let (next, loc, hold) = eof_continue(&bar.tl, &p3, 1097.0, "ch3 eof continue");
        assert!(crate::video_ext::paths_same_file(&next, &p4));
        assert!(loc < 5.0, "local={loc}");
        let expected_hold = (bar.tl.starts[i3] + 1097.0 + 0.05).min(bar.tl.total_sec);
        assert!(
            (hold - expected_hold).abs() < 0.1,
            "hold={hold} expected={expected_hold}"
        );
    }

    /// Synthetic four-chapter durations plus the chapter-1/chapter-4 paths.
    fn dvd4_synthetic_map(
        vob: &std::path::Path,
    ) -> (HashMap<String, f64>, std::path::PathBuf, std::path::PathBuf) {
        (
            dur_map(&[
                (vob, 1102.0),
                (&vob.with_file_name("VTS_02_2.VOB"), 1103.0),
                (&vob.with_file_name("VTS_02_3.VOB"), 1098.0),
                (&vob.with_file_name("VTS_02_4.VOB"), 924.0),
            ]),
            vob.to_path_buf(),
            vob.with_file_name("VTS_02_4.VOB"),
        )
    }

    fn assert_dvd4_resume_lands_in_ch4(bar_tl: &DvdVobTimeline, vob: &std::path::Path) {
        let (map, p1, p4) = dvd4_synthetic_map(vob);
        let resume_g = bar_tl.starts[bar_tl.index_of(&p4).expect("p4 idx")] + 5.0;
        let still = crate::dvd_entity::still_at_global(&p1, resume_g, &map, None, None)
            .expect("ch4 resume");
        assert!(
            crate::video_ext::paths_same_file(&still.load, &p4),
            "resume should open ch4, got {}",
            still.load.display()
        );
        assert!(still.local_sec < 10.0, "local={}", still.local_sec);
        assert_dvd4_timeline_resumes_in_ch4(&p1, &map, resume_g, &p4);
    }

    fn assert_dvd4_timeline_resumes_in_ch4(
        p1: &std::path::Path,
        map: &HashMap<String, f64>,
        resume_g: f64,
        p4: &std::path::Path,
    ) {
        let tl = crate::dvd_entity::build_title_timeline(p1, map, 1102.0).expect("tl");
        let (idx, local) = tl.resolve_global(resume_g);
        assert!(
            crate::video_ext::paths_same_file(tl.path_at(idx).expect("idx"), p4),
            "resume should map to ch4, got idx={idx} local={local:.2}"
        );
        assert!(local < 10.0, "local={local}");
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd4_mounted_ch3_eof_advances_to_ch4() {
        let Some(vob) = mounted_vob(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD4/Video_ts/VTS_02_1.VOB",
        ) else {
            return;
        };
        let bar = dvd4_full_bar(&vob);
        assert_ch3_eof_continues_to_ch4(&bar, &vob);
        assert_dvd4_resume_lands_in_ch4(&bar.tl, &vob);
    }

    /// Synthetic-map timeline with real IFO chapter labels attached.
    fn dvd4_label_bar(p1: &std::path::Path, map: &HashMap<String, f64>) -> DvdBarState {
        let tl = crate::dvd_entity::build_title_timeline(p1, map, 1102.0).expect("tl");
        let labels = chapter_labels_for_timeline(&tl);
        assert!(
            labels.len() >= 2,
            "expected IFO chapter marks within VTS_02, got {}",
            labels.len()
        );
        DvdBarState {
            tl,
            chapter_labels: labels,
        }
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn dvd4_multi_ifo_preview_caps_at_chapter_marks() {
        let Some(vob) = mounted_vob(
            "/Volumes/SanDisk/Torrents/17_Mgnoveniy_vesni/17_Mgnoveniy_DVD4/Video_ts/VTS_02_1.VOB",
        ) else {
            return;
        };
        let (map, p1, p4) = dvd4_synthetic_map(&vob);
        let bar = dvd4_label_bar(&p1, &map);
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

    /// Fritt.vilt IFO bar totals ~97 min with a ~1062 s first segment.
    fn assert_fritt_bar_shape(bar: &DvdBarState) {
        assert!(
            (bar.total_sec() - 5842.0).abs() < 5.0,
            "bar total {:.1}s",
            bar.total_sec()
        );
        assert!(
            bar.chapter_dur_at(0) > 1050.0 && bar.chapter_dur_at(0) < 1080.0,
            "VTS_01_1 ~1062s, got {:.1}s",
            bar.chapter_dur_at(0)
        );
    }

    /// The open chapter maps back onto itself on the whole-title bar.
    fn assert_current_vob_resolves(bar: &DvdBarState, vob: &std::path::Path) {
        let (idx6, local6) = bar.resolve_global(bar.global_pos(vob, 0.0));
        assert_eq!(idx6, bar.tl.index_of(vob).expect("idx"));
        assert!(local6.abs() < 1.0);
    }

    /// Skips when the local sample rip is not mounted.
    #[test]
    fn fritt_resume_chapter6_seek_to_start() {
        let Some(vob) =
            mounted_vob("/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_6.VOB")
        else {
            return;
        };
        let bar = DvdBarState::build_with_map(&vob, 1072.0, &HashMap::new()).expect("bar");
        assert_fritt_bar_shape(&bar);
        let (idx, local) = bar.resolve_global(0.0);
        assert_eq!(idx, 0);
        assert!(local.abs() < 1e-6);
        assert_eq!(
            bar.path_at(idx)
                .and_then(|p| p.file_name().and_then(|n| n.to_str())),
            Some("VTS_01_1.VOB")
        );
        assert_fritt_preview_dur(&bar, 0.0, bar.chapter_dur_at(0) * 0.95);
        assert_fritt_preview_dur(&bar, 500.0, 900.0);
        assert_current_vob_resolves(&bar, &vob);
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
        assert!((chain_head_ifo_local_to_mpv(520.0, dur, seg, true) - (tail + 520.0)).abs() < 1e-6);
        assert!((chain_head_ifo_local_to_mpv(520.0, dur, seg, false) - 520.0).abs() < 1e-6);
        assert!((chain_head_ifo_local_to_mpv(1062.0, dur, seg, true) - dur).abs() < 0.1);
    }

    #[test]
    fn chain_ifo_local_from_mpv_tail() {
        let seg = 1062.0;
        let dur = 90_658.0;
        let tail = dur - seg;
        assert!((super::chain_head_ifo_local_from_mpv(520.0, dur, seg) - 520.0).abs() < 1e-6);
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
        assert!((chain_head_ifo_local_to_mpv(419.0, dur, seg, true) - (tail + 419.0)).abs() < 1e-6);
        assert!((chain_head_ifo_local_to_mpv(419.0, dur, seg, false) - 419.0).abs() < 1e-6);
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
        let (base, vts) = temp_vts("persist");
        write_vobs(&vts, &["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        let p1 = vts.join("VTS_01_1.VOB");
        let map = dur_map(&[(&p1, 1062.0), (&vts.join("VTS_01_2.VOB"), 1069.0)]);
        let tl = DvdVobTimeline::from_title_vobs(&p1, &map, 1062.0).expect("tl");
        let mpv_dur = 90_658.0;
        let mpv_pos = mpv_dur - 1062.0 + 125.73;
        let local = timeline_local_from_mpv(&tl, &p1, mpv_pos, mpv_dur);
        assert!(
            (local - 125.73).abs() < 0.1,
            "ifo local from virtual tail, got {local}"
        );
        assert!(
            (tl.global_pos(&p1, local) - 125.73).abs() < 0.1,
            "global must not clamp to title end"
        );
        assert_persist_snapshot(&p1, mpv_pos, mpv_dur, &map);
        rm_rf(&base);
    }

    /// Persisted playback snapshot keeps the whole-title global position.
    fn assert_persist_snapshot(
        p1: &std::path::Path,
        mpv_pos: f64,
        mpv_dur: f64,
        map: &HashMap<String, f64>,
    ) {
        let snap = crate::dvd_entity::playback_snapshot(p1, mpv_pos, mpv_dur, map).expect("snap");
        assert!(
            (snap.1 - 125.73).abs() < 0.1,
            "persist snapshot global, got {}",
            snap.1
        );
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
