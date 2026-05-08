/// Set after **`user-data=`** is rejected once but plain **`vapoursynth:`** succeeds (Ubuntu **mpv 0.38**
/// has no `user-data`; **mpv** **master** does — see **`vf_vapoursynth`** `vf_opts_fields`).
static VAPOURSYNTH_USER_DATA_UNSUPPORTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Attaches **`vf=vapoursynth`** (**`buffered-frames`** fixed).
/// With **`Some(px²)`** (**`Option<u64>`**, bundled script only), tries **`user-data=<cap>`** first so newer
/// **mpv** exposes **`user_data`** in the worker. Older **mpv** rejects unknown sub-options — then Rhino retries
/// without **`user-data=`** (**`RHINO_SMOOTH_MAX_AREA`** still set for the bundled **`.vpy`**). Custom scripts use **`None`**.
fn smooth_vapoursynth_vf_try_attach(
    mpv: &libmpv2::Mpv,
    script_path_escaped: &str,
    me_budget_px: Option<u64>,
) -> bool {
    let bf = SMOOTH_VF_BUFFERED_FRAMES;
    let spec_plain = format!(
        "vapoursynth:file={script_path_escaped}:buffered-frames={bf}:concurrent-frames=auto",
    );
    if let Some(px) = me_budget_px {
        if !VAPOURSYNTH_USER_DATA_UNSUPPORTED.load(std::sync::atomic::Ordering::Relaxed) {
            let cap = px.max(crate::db::MIN_SMOOTH_MAX_AREA);
            let spec_ud = format!("{spec_plain}:user-data={cap}");
            if vapoursynth_vf_try_one_spec(mpv, &spec_ud) {
                return true;
            }
            if vapoursynth_vf_try_one_spec(mpv, &spec_plain) {
                VAPOURSYNTH_USER_DATA_UNSUPPORTED.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "[rhino] video: libmpv does not accept vapoursynth `user-data=` (e.g. mpv 0.38); using env-only ME cap — upgrade mpv for in-worker budget sync"
                );
                return true;
            }
            return false;
        }
    }
    vapoursynth_vf_try_one_spec(mpv, &spec_plain)
}

fn vapoursynth_vf_try_one_spec(mpv: &libmpv2::Mpv, spec: &str) -> bool {
    match mpv.command("vf", &["add", spec]) {
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
    match mpv.set_property("vf", spec) {
        Ok(()) => {
            eprintln!(
                "[rhino] video: VapourSynth set via `vf` property (fallback after vf add error)"
            );
            true
        }
        Err(e2) => {
            eprintln!("[rhino] video: set_property vf fallback failed: {e2:?}");
            false
        }
    }
}
