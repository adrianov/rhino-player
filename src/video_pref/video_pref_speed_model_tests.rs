#[cfg(test)]
mod model_tests {
    //! [super::mvtools_vf_eligible] is the source of truth; this module mirrors the **speed** part so
    //! tests do not need an [Mpv] handle.

    use super::normalized_env_speed;
    use super::PLAYBACK_1X_EPS;

    fn mvtools_vf_wanted_for_speed(s: f64) -> bool {
        let t = normalized_env_speed(s);
        (t - 1.0).abs() <= PLAYBACK_1X_EPS
    }

    /// When the graph **should** include `vapoursynth` (pref on + ~1.0×) but the string does not, an
    /// [apply_mpv_video] (or [super::reapply_60_if_still_missing] after the post-load timer) fixes it.
    fn graph_lacks_script_while_wanted(
        smooth_pref: bool,
        playback_speed: f64,
        vf_has_vapoursynth: bool,
    ) -> bool {
        smooth_pref && mvtools_vf_wanted_for_speed(playback_speed) && !vf_has_vapoursynth
    }

    #[test]
    fn bundled_script_only_at_1x() {
        assert!(mvtools_vf_wanted_for_speed(1.0));
        assert!(!mvtools_vf_wanted_for_speed(1.5));
        assert!(!mvtools_vf_wanted_for_speed(2.0));
        assert!(!mvtools_vf_wanted_for_speed(8.0));
    }

    #[test]
    fn sped_up_does_not_require_vapoursynth_in_vf() {
        assert!(!graph_lacks_script_while_wanted(true, 1.5, false));
        assert!(!graph_lacks_script_while_wanted(true, 2.0, false));
        assert!(!graph_lacks_script_while_wanted(true, 8.0, false));
    }

    #[test]
    fn at_1x_pref_on_missing_vf_is_stale_graph() {
        assert!(graph_lacks_script_while_wanted(true, 1.0, false));
        assert!(!graph_lacks_script_while_wanted(true, 1.0, true));
        assert!(!graph_lacks_script_while_wanted(false, 1.0, false));
    }
}
