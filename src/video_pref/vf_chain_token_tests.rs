#[cfg(test)]
mod vf_chain_token_tests {
    use super::vf_concurrent_frames_matches;

    #[test]
    fn concurrent_frames_auto_ok() {
        let vf = "vapoursynth:file=/x.vpy:buffered-frames=4:concurrent-frames=auto";
        assert!(vf_concurrent_frames_matches(vf, "auto"));
    }

    #[test]
    fn concurrent_frames_prefix_digit_not_confused() {
        assert!(!vf_concurrent_frames_matches(
            "concurrent-frames=120",
            "12"
        ));
    }
}
