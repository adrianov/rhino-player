/// Attaches **`vf=vapoursynth`** with fixed **`buffered-frames`** and **`concurrent-frames=auto`**.
///
/// Bundled ME px² is passed only via **`RHINO_SMOOTH_CAP_FILE`** (**[crate::paths::publish_smooth_me_cap_snap]**),
/// which the `.vpy` reads — not via **`vf`** sub-options (mpv builds vary; forked workers may miss env).
pub(crate) fn smooth_vapoursynth_vf_try_attach(mpv: &libmpv2::Mpv, script_path_escaped: &str) -> bool {
    let bf = SMOOTH_VF_BUFFERED_FRAMES;
    let spec = format!(
        "vapoursynth:file={script_path_escaped}:buffered-frames={bf}:concurrent-frames=auto",
    );
    match mpv.command("vf", &["add", spec.as_str()]) {
        Ok(()) => {
            eprintln!("[rhino] video: vf add vapoursynth command accepted");
            true
        }
        Err(e) => {
            eprintln!(
                "[rhino] video: vf add vapoursynth failed: {e:?} (install VapourSynth + mvtools if this persists)."
            );
            false
        }
    }
}
