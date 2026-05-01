/// True when mpv's `vf` chain already matches what [add_smooth_60] would install for current prefs
/// (same script path and frame-queue settings). Used to skip redundant **`vf clr`**/**`vf add`** when
/// transport fires duplicate idle callbacks after **FileLoaded** / **`path`** — **seek** never reaches
/// [apply_mpv_video_impl].
pub(crate) fn smooth_vf_matches_loaded_prefs(mpv: &Mpv, v: &VideoPrefs) -> bool {
    if !v.smooth_60 {
        return false;
    }
    let Some(script) = resolve_vs_script_path(v) else {
        return false;
    };
    let Ok(vf) = mpv.get_property::<String>("vf") else {
        return false;
    };
    let vfl = vf.to_lowercase();
    if !vfl.contains("vapoursynth") {
        return false;
    }
    let bf = format!("buffered-frames={VS_BUFFERED_FRAMES}");
    if !vf.contains(&bf) || !vf.contains("concurrent-frames=auto") {
        return false;
    }
    let script = script.trim();
    let esc = mpv_escape_path(script);
    let path_matches = vf.contains(&esc) || vf.contains(script);
    // mpv may rewrite `file=` (percent escapes, brackets, tilde) so the property string does not
    // contain our raw path — matching the script basename avoids spurious `vf clr` / rebuild cycles.
    let base_matches = std::path::Path::new(script)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|base| vf.contains(base));
    path_matches || base_matches
}
