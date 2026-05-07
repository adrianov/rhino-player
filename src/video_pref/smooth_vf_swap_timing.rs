// Linux: swap reports only while **`display-resample`** + atomic gate. macOS: gate true for **`display-resample`** plain + Smooth.

use std::sync::atomic::AtomicBool;

/// Gate for `mpv_render_context_report_swap` on **Linux** (**`GLArea`** draw path): enable while
/// **`video-sync=display-resample`** (Smooth **on**). Disable **only after** **`video-sync`** has switched to
/// **`audio`** so mpv never runs **`display-resample`** without swap timing (**`vo=libmpv`** pacing collapses).
///
/// **macOS:** plain playback also uses **`display-resample`** + **`report_swap`** ( **`CVDisplayLink`** ); the gate
/// stays **true** whenever **`restore_non_smooth_present_opts`** applied **`display-resample`** (fallback to **`audio`**
/// clears it).
///
/// **`SeqCst`**: update callback / **`CVDisplayLink`** thread vs GTK **`vf clr`** — avoids **`report_swap`** racing teardown.
static SMOOTH_VF_TIMING_REPORT: AtomicBool = AtomicBool::new(false);

pub(crate) fn smooth_vf_timing_report_active() -> bool {
    SMOOTH_VF_TIMING_REPORT.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) fn smooth_vf_swap_timing_set(active: bool) {
    SMOOTH_VF_TIMING_REPORT.store(active, std::sync::atomic::Ordering::SeqCst);
}
