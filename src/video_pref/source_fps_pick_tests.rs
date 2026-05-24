    use super::mask_est_for_path_change_with_state;
    use super::mpv_path_is_disc;
    use super::source_fps_from_container_and_estimated;
    use super::stabilize_disc_source_fps;
    use super::sticky_local_source_fps;
    use super::update_interleaved_cadence_gate;
    use super::CADENCE_STABLE_READS;
    use super::FpsPickGateState;

    #[test]
    fn ntsc_film_prefers_estimated_when_container_rounds_to_24() {
        let ntsc = 24000.0 / 1001.0;
        assert!((source_fps_from_container_and_estimated(Some(24.0), Some(ntsc)).unwrap() - ntsc).abs() < 1e-6);
        assert!(
            (source_fps_from_container_and_estimated(Some(24.0), Some(24.0)).unwrap() - 24.0).abs() < 1e-6
        );
    }

    #[test]
    fn container_only_passthrough_including_24() {
        assert!((source_fps_from_container_and_estimated(Some(24.0), None).unwrap() - 24.0).abs() < 1e-6);
    }

    #[test]
    fn container_passthrough_when_no_estimate_non_24() {
        assert!(
            (source_fps_from_container_and_estimated(Some(29.97), None).unwrap() - 29.97).abs() < 1e-6
        );
    }

    #[test]
    fn container_passthrough_when_no_mismatch_with_estimate() {
        assert!(
            (source_fps_from_container_and_estimated(Some(29.97), Some(29.97)).unwrap() - 29.97).abs()
                < 1e-6
        );
    }

    #[test]
    fn fps_gate_skips_est_after_path_change_burst() {
        let mut g = FpsPickGateState::default();
        let ntsc = 24000.0 / 1001.0;
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/a".into()), Some(ntsc), &mut g, None),
            Some(ntsc)
        );
    }

    #[test]
    fn interleaved_jump_keeps_display_resample_mode() {
        let mut g = FpsPickGateState::default();
        let path: Option<String> = Some("bd://1".into());
        let film = 24000.0 / 1001.0;
        let video = 30000.0 / 1001.0;
        for _ in 0..CADENCE_STABLE_READS {
            update_interleaved_cadence_gate(path.as_deref(), Some(film), &mut g);
        }
        assert!(!g.interleaved_smooth);
        update_interleaved_cadence_gate(path.as_deref(), Some(video), &mut g);
        assert!(g.interleaved_smooth);
    }

    #[test]
    fn disc_stabilizer_ignores_wild_est_after_plausible_container() {
        let mut g = FpsPickGateState::default();
        let path: Option<String> = Some("bd://1".into());
        let first = stabilize_disc_source_fps(path.as_deref(), Some(24000.0 / 1001.0), &mut g);
        assert!(first.is_some());
        assert_eq!(g.locked_disc_fps, first);
        assert_eq!(
            stabilize_disc_source_fps(path.as_deref(), Some(60.0), &mut g),
            first
        );
        assert_eq!(stabilize_disc_source_fps(path.as_deref(), Some(6.5), &mut g), first);
    }

    #[test]
    fn local_missing_mpv_read_keeps_sticky_cadence() {
        let mut g = FpsPickGateState::default();
        let ntsc = 30000.0 / 1001.0;
        g.last_stable_fps = Some(ntsc);
        assert!((sticky_local_source_fps(&g).unwrap() - ntsc).abs() < 1e-6);
        update_interleaved_cadence_gate(None, None, &mut g);
        assert!(!g.interleaved_smooth);
    }

    #[test]
    fn disc_missing_mpv_read_stays_interleaved() {
        let mut g = FpsPickGateState::default();
        let path: Option<String> = Some("bd://1".into());
        g.last_stable_fps = Some(24000.0 / 1001.0);
        update_interleaved_cadence_gate(path.as_deref(), None, &mut g);
        assert!(g.interleaved_smooth);
    }

    #[test]
    fn mpv_path_is_disc_helper() {
        assert!(mpv_path_is_disc("bd://foo"));
        assert!(mpv_path_is_disc("bluray://bar"));
        assert!(mpv_path_is_disc("dvd://1"));
        assert!(!mpv_path_is_disc("/movie.mkv"));
    }

    #[test]
    fn fps_gate_drops_stale_ntsc_after_opening_true_24_file() {
        let mut g = FpsPickGateState::default();
        let ntsc = 24000.0 / 1001.0;
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/sp.mkv".into()), Some(ntsc), &mut g, None),
            Some(ntsc)
        );
        for _ in 0..super::FPS_EST_IGNORE_READS_AFTER_PATH_CHANGE {
            assert_eq!(
                mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(ntsc), &mut g, None),
                None
            );
        }
        assert_eq!(
            mask_est_for_path_change_with_state(Some("/holmes.mkv".into()), Some(24.0), &mut g, None),
            Some(24.0)
        );
        assert!((source_fps_from_container_and_estimated(Some(24.0), Some(24.0)).unwrap() - 24.0).abs()
            < 1e-6);
    }
