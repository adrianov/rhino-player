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

    #[test]
    fn vapoursynth_remove_specs_from_property_string() {
        use super::vapoursynth_vf_specs_from_property;
        let vf = "vapoursynth=file=%2Ftmp/x.vpy:buffered-frames=4:concurrent-frames=auto";
        let specs = vapoursynth_vf_specs_from_property(vf);
        assert!(specs.contains(&vf.to_string()));
        assert!(specs.iter().any(|s| s.starts_with("vapoursynth:file=")));
    }
}
