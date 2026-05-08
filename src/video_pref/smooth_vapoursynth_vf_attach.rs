/// Attaches bundled **`vf=vapoursynth`**. Tries **`user-data=`** (modern libmpv); on rejection, retries
/// without **`user-data`** and **jittered** **`buffered-frames`** so the chain string tracks the ME budget.
fn smooth_vapoursynth_vf_try_attach(
    mpv: &libmpv2::Mpv,
    script_path_escaped: &str,
    smooth_area_px: u64,
) -> bool {
    let bf_modern = SMOOTH_VF_BUFFERED_FRAMES;
    let bf_legacy = smooth_vf_buffer_legacy_depth(smooth_area_px);
    let area_tag = mpv_fixed_len_quote(&format!("{smooth_area_px}"));
    let spec_modern = format!(
        "vapoursynth:file={script_path_escaped}:buffered-frames={bf_modern}:concurrent-frames=auto:user-data={area_tag}"
    );
    let spec_legacy = format!(
        "vapoursynth:file={script_path_escaped}:buffered-frames={bf_legacy}:concurrent-frames=auto"
    );

    if video_log() {
        eprintln!(
            "[rhino] video: (verbose) smooth vf try modern buffered-frames={bf_modern}, legacy fallback uses {bf_legacy}"
        );
    }

    match mpv.command("vf", &["add", spec_modern.as_str()]) {
        Ok(()) => {
            eprintln!("[rhino] video: vf add vapoursynth command accepted");
            return true;
        }
        Err(e1) => {
            eprintln!(
                "[rhino] video: vf add vapoursynth failed: {e1:?} (trying set_property; install VapourSynth + mvtools if this persists)."
            );
            match mpv.set_property("vf", spec_modern.as_str()) {
                Ok(()) => {
                    eprintln!(
                        "[rhino] video: VapourSynth set via `vf` property (fallback after vf add error)"
                    );
                    return true;
                }
                Err(e2) => {
                    eprintln!(
                        "[rhino] video: `vapoursynth:user-data` not accepted by this libmpv ({e2:?}) — retrying without `user-data` (buffered-frames={bf_legacy}; ME budget from RHINO_SMOOTH_MAX_AREA only)"
                    );
                }
            }
        }
    }

    match mpv.command("vf", &["add", spec_legacy.as_str()]) {
        Ok(()) => {
            eprintln!("[rhino] video: vf add vapoursynth command accepted");
            true
        }
        Err(e3) => {
            eprintln!(
                "[rhino] video: vf add vapoursynth (legacy vf string) failed: {e3:?} (trying set_property)"
            );
            match mpv.set_property("vf", spec_legacy.as_str()) {
                Ok(()) => {
                    eprintln!(
                        "[rhino] video: VapourSynth set via `vf` property after legacy vf string"
                    );
                    true
                }
                Err(e4) => {
                    eprintln!("[rhino] video: set_property vf fallback failed: {e4:?}");
                    false
                }
            }
        }
    }
}
