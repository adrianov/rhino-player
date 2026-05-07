// Atomic gate for `mpv_render_context_report_swap` — must stay aligned with **`video-sync=display-resample`**.

use std::sync::atomic::AtomicBool;

/// Gate **`mpv_render_context_report_swap`** (Linux **`GLArea`** / macOS **`CAOpenGLLayer`**): **true** when
/// **`set_property("video-sync", "display-resample")`** succeeded (**`restore_non_smooth_present_opts`** or Smooth **`vf`**).
/// **false** after **`audio`** fallback or after **`restore`** switches to **`audio`** before clearing the gate.
///
/// **`vf clr`**: do **not** clear the gate before **`vf`** is emptied — **never** **`display-resample`** without swaps
/// during teardown (**`SeqCst`** coordinates GTK main vs **`CVDisplayLink`**).
static SMOOTH_VF_TIMING_REPORT: AtomicBool = AtomicBool::new(false);

pub(crate) fn smooth_vf_timing_report_active() -> bool {
    SMOOTH_VF_TIMING_REPORT.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) fn smooth_vf_swap_timing_set(active: bool) {
    SMOOTH_VF_TIMING_REPORT.store(active, std::sync::atomic::Ordering::SeqCst);
}
