/// Fixed mpv **`vf vapoursynth:`** frame-queue depth (smaller → less vf RAM / prefetch; MVTools cost is
/// dominated by the `.vpy` graph). Spatial MVTools preset stays inside **`video_in`** tiers in the
/// bundled script.
pub(crate) const SMOOTH_VF_BUFFERED_FRAMES: i32 = 16;
