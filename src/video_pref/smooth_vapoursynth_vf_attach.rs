/// Attaches **`vf=vapoursynth`** (`buffered-frames` fixed; ME cap from **`RHINO_SMOOTH_MAX_AREA`** — bundled `.vpy`).
fn smooth_vapoursynth_vf_try_attach(mpv: &libmpv2::Mpv, script_path_escaped: &str) -> bool {
    let bf = SMOOTH_VF_BUFFERED_FRAMES;
    let spec =
        format!("vapoursynth:file={script_path_escaped}:buffered-frames={bf}:concurrent-frames=auto");

    match mpv.command("vf", &["add", spec.as_str()]) {
        Ok(()) => {
            eprintln!("[rhino] video: vf add vapoursynth command accepted");
            return true;
        }
        Err(e1) => {
            eprintln!(
                "[rhino] video: vf add vapoursynth failed: {e1:?} (trying set_property; install VapourSynth + mvtools if this persists)."
            );
        }
    }
    match mpv.set_property("vf", spec.as_str()) {
        Ok(()) => {
            eprintln!("[rhino] video: VapourSynth set via `vf` property (fallback after vf add error)");
            true
        }
        Err(e2) => {
            eprintln!("[rhino] video: set_property vf fallback failed: {e2:?}");
            false
        }
    }
}
