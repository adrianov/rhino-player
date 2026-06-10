#[cfg(test)]
mod smooth_vf_lifecycle_tests {
    use super::vf_resync_sec_from_sources;

    #[test]
    fn resync_prefers_pending_resume_over_early_time_pos() {
        let t = vf_resync_sec_from_sources(Some(2785.366), Some(0.04), Some(0.0));
        assert_eq!(t, Some(2785.366));
    }

    #[test]
    fn resync_falls_back_to_playback_time_without_pending() {
        let t = vf_resync_sec_from_sources(None, Some(120.5), Some(0.0));
        assert_eq!(t, Some(120.5));
    }

    #[test]
    fn resync_uses_time_pos_when_playback_time_missing() {
        let t = vf_resync_sec_from_sources(None, None, Some(42.0));
        assert_eq!(t, Some(42.0));
    }

    #[test]
    fn resync_none_when_all_sources_invalid() {
        assert_eq!(vf_resync_sec_from_sources(None, None, None), None);
        assert_eq!(
            vf_resync_sec_from_sources(Some(f64::NAN), Some(-1.0), None),
            None
        );
    }
}
