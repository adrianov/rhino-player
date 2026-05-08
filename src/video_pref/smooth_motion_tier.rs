/// Fixed mpv **`vf vapoursynth:`** frame-queue depth (smaller → less vf RAM / prefetch; MVTools cost is
/// dominated by the `.vpy` graph). Spatial MVTools preset stays inside **`video_in`** tiers in the
/// bundled script.
pub(crate) const SMOOTH_VF_BUFFERED_FRAMES: i32 = 16;

/// When **`vapoursynth:user-data=`** is unavailable (older libmpv), the vf string must still change
/// when **`video_smooth_max_area`** changes or mpv may keep a stale VapourSynth graph. Jitter
/// **`buffered-frames`** in **15…17** from the saved area (default path uses stable **16** + **`user-data`**).
pub(crate) fn smooth_vf_buffer_legacy_depth(smooth_max_area_px: u64) -> i32 {
    15 + ((smooth_max_area_px % 3) as i32)
}

#[cfg(test)]
mod smooth_buffer_depth_tests {
    use super::*;

    #[test]
    fn legacy_depth_ranges_15_through_17() {
        assert_eq!(smooth_vf_buffer_legacy_depth(0), 15);
        assert_eq!(smooth_vf_buffer_legacy_depth(1), 16);
        assert_eq!(smooth_vf_buffer_legacy_depth(2), 17);
        assert_eq!(smooth_vf_buffer_legacy_depth(3), 15);
    }
}
