/// **`concurrent-frames=<want>`** without accepting a longer numeric prefix (**`…=12`** vs **`…=120`**).
fn vf_concurrent_frames_matches(vf: &str, want: &str) -> bool {
    let needle = format!("concurrent-frames={want}");
    for (idx, _) in vf.match_indices(&needle) {
        let tail = &vf[idx + needle.len()..];
        let next = tail.chars().next();
        if matches!(next, Some(c) if c.is_ascii_digit()) {
            continue;
        }
        return true;
    }
    false
}
