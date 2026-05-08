mod budget_tests {
    use super::*;

    #[test]
    fn recovery_raises_by_ten_percent_and_caps_default() {
        assert_eq!(
            recovery_candidate(500_000_u64),
            Some(clamp_smooth_area(550_000_u64))
        );
        let just_below_default = crate::db::DEFAULT_SMOOTH_MAX_AREA - 800;
        assert_eq!(
            recovery_candidate(just_below_default),
            Some(crate::db::DEFAULT_SMOOTH_MAX_AREA)
        );
    }

    #[test]
    fn recovery_unknown_at_nominal_ceiling() {
        assert_eq!(recovery_candidate(crate::db::DEFAULT_SMOOTH_MAX_AREA), None);
    }

    #[test]
    fn drop_rate_four_percent_budget_halves_about() {
        let r = 0.04_f64;
        assert!(r > DROP_OVERLOAD_FRAC + 1e-6);
        assert_eq!(
            budget_after_decoder_overload(800_000, r),
            clamp_smooth_area(400_000)
        );
    }

    #[test]
    fn budget_decoder_overload_clamps_near_min() {
        let x =
            budget_after_decoder_overload(crate::db::MIN_SMOOTH_MAX_AREA + 9_999, DROP_OVERLOAD_FRAC * 3.0);
        assert!(x >= crate::db::MIN_SMOOTH_MAX_AREA);
    }

    #[test]
    fn overload_spike_rate_cap_and_half_floor() {
        assert_eq!(
            budget_after_decoder_overload(800_000, 10.0),
            clamp_smooth_area(400_000)
        );
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
