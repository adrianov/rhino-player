mod budget_tests {
    use super::*;

    #[test]
    fn recovery_raises_by_ten_percent_and_caps_default_when_decode_unknown() {
        assert_eq!(
            recovery_candidate(500_000_u64, None),
            Some(clamp_smooth_area(550_000_u64))
        );
        let just_below_default = crate::db::DEFAULT_SMOOTH_MAX_AREA - 800;
        assert_eq!(
            recovery_candidate(just_below_default, None),
            Some(crate::db::DEFAULT_SMOOTH_MAX_AREA)
        );
    }

    #[test]
    fn recovery_unknown_decode_stops_at_default_reference() {
        assert_eq!(
            recovery_candidate(crate::db::DEFAULT_SMOOTH_MAX_AREA, None),
            None
        );
    }

    #[test]
    fn recovery_ceiling_follows_decode_when_wider_than_default() {
        let decode = 6144000_u64;
        assert_eq!(recovery_ceiling_px(Some(decode)), decode);
        assert_eq!(
            recovery_candidate(crate::db::DEFAULT_SMOOTH_MAX_AREA, Some(decode)),
            Some(2_280_960_u64)
        );
        assert_eq!(recovery_candidate(decode, Some(decode)), None);
    }

    #[test]
    fn overload_step_down_matches_recovery_step_ratio() {
        assert_eq!(
            budget_after_decoder_overload(800_000, 0.04),
            clamp_smooth_area(720_000)
        );
        assert_eq!(
            budget_after_decoder_overload(800_000, 10.0),
            clamp_smooth_area(720_000)
        );
    }

    #[test]
    fn overload_down_clamps_at_min_area() {
        let floor = crate::db::MIN_SMOOTH_MAX_AREA;
        assert_eq!(budget_after_decoder_overload(floor, 1.0), floor);
        let hi = floor.saturating_add(99_999);
        let x = budget_after_decoder_overload(hi, 1.0);
        assert!(x >= floor && x < hi);
    }

    #[test]
    fn hz_mistimed_matches_display_not_decode_only_fps() {
        use SmoothBudgetSignalSrc::*;
        let secs = 5.0_f64;
        let delta = 120_u64;
        let hz_m = budget_signal_hz_for_comparison(24.0_f64, Mistimed);
        let hz_d = budget_signal_hz_for_comparison(24.0_f64, DecoderDrop);
        assert!((hz_m - 60.0).abs() < 1e-6);
        assert!((hz_d - 24.0).abs() < 1e-6);
        let rm = budget_signal_rate_in_window(delta, secs, hz_m);
        let rd = budget_signal_rate_in_window(delta, secs, hz_d);
        assert!((rm * 300.0 - rd * 120.0).abs() < 1e-3);
    }

    #[test]
    fn recovery_raise_only_when_decode_exceeds_me_budget() {
        assert!(!raised_me_budget_can_reduce_downscale(
            Some(800_000),
            800_000
        ));
        assert!(!raised_me_budget_can_reduce_downscale(
            Some(800_000),
            900_000
        ));
        assert!(raised_me_budget_can_reduce_downscale(
            Some(800_001),
            800_000
        ));
        assert!(raised_me_budget_can_reduce_downscale(None, 500_000));
    }
}
