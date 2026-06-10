/// Split mpv **`vf`** property text into top-level filter entries (comma-separated).
fn vf_property_entries(vf: &str) -> Vec<String> {
    let vf = vf.trim();
    if vf.is_empty() {
        return vec![];
    }
    vf.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// **`vf remove`** targets for each **vapoursynth** node in the property string.
///
/// With **`vo=libmpv`**, removing by label **`vapoursynth`** alone is a no-op; mpv needs the full
/// **`vapoursynth=file=…`** entry returned by **`get_property vf`** (see IPC repro in feature 26 Notes).
pub(crate) fn vapoursynth_vf_specs_from_property(vf: &str) -> Vec<String> {
    let mut out = Vec::new();
    for entry in vf_property_entries(vf) {
        if !entry.to_ascii_lowercase().starts_with("vapoursynth") {
            continue;
        }
        out.push(entry.clone());
        if let Some(rest) = entry.strip_prefix("vapoursynth=") {
            let alt = format!("vapoursynth:{rest}");
            if !out.contains(&alt) {
                out.push(alt);
            }
        }
    }
    out
}

pub(crate) fn vapoursynth_vf_remove_specs(mpv: &libmpv2::Mpv) -> Vec<String> {
    mpv.get_property::<String>("vf")
        .map(|s| vapoursynth_vf_specs_from_property(&s))
        .unwrap_or_default()
}

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
