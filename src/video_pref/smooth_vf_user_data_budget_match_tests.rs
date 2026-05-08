#[cfg(test)]
mod vf_user_data_budget_match_tests {
    use super::vf_bundled_user_data_budget_ok;

    #[test]
    fn user_data_budget_ok_legacy_vf_without_key() {
        assert!(
            vf_bundled_user_data_budget_ok("vapoursynth:file=/x.vpy:buffered-frames=4", 723_100)
        );
    }

    #[test]
    fn user_data_exact_cap_matches() {
        let vf = "...:buffered-frames=4:concurrent-frames=auto:user-data=723100";
        assert!(vf_bundled_user_data_budget_ok(vf, 723_100));
    }

    #[test]
    fn user_data_must_not_partial_match_prefix_digit() {
        assert!(!vf_bundled_user_data_budget_ok(":user-data=723100", 72));
    }
}
