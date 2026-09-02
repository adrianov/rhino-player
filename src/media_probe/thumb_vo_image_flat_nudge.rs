// Flat-still nudge: if the primary capture is almost uniform, retry later seeks
// at doubling offsets (+1s, +2s, +4s, …) until a detailed frame or the duration cap.

use libmpv2::Mpv;
use std::path::Path;

use super::vo_image_capture_after_seek;

/// Largest forward offset from the primary still (doubling stops here).
const FLAT_NUDGE_MAX_SEC: f64 = 64.0;

pub(super) struct FlatNudgeCtx<'a> {
    pub m: &'a mut Mpv,
    pub src: &'a Path,
    pub ifo_seek: f64,
    pub cap: f64,
    pub chain_head: bool,
    pub dvd_vob: bool,
    pub wait_secs: u64,
}

/// If `first` is almost uniform, recapture later; keep `first` only when every probe is flat.
pub(super) fn vo_image_prefer_nonflat(ctx: FlatNudgeCtx<'_>, first: Vec<u8>) -> Option<Vec<u8>> {
    if !crate::thumb_texture::thumb_webp_is_flat_fill(&first) {
        return Some(first);
    }
    eprintln!(
        "[rhino] grid_thumb flat still at {:.2}s; trying later seeks {}",
        ctx.ifo_seek,
        ctx.src.display()
    );
    for nudged in flat_nudge_seeks(ctx.ifo_seek, ctx.cap) {
        // Exact seek: keyframe snaps would collapse +1/+2/+4 onto the same black GOP.
        let Some(b) = vo_image_capture_after_seek(
            ctx.m,
            ctx.src,
            nudged,
            ctx.chain_head,
            ctx.dvd_vob,
            ctx.wait_secs,
            false,
        ) else {
            continue;
        };
        if crate::thumb_texture::thumb_webp_is_flat_fill(&b) {
            continue;
        }
        eprintln!(
            "[rhino] grid_thumb flat nudge ok {:.2}->{nudged:.2} {}",
            ctx.ifo_seek,
            ctx.src.display()
        );
        return Some(b);
    }
    Some(first)
}

fn flat_nudge_seeks(base: f64, cap: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut step = 1.0;
    while step <= FLAT_NUDGE_MAX_SEC {
        let t = crate::seek_bar_preview::cap_preview_seek_time(base + step, cap);
        if (t - base).abs() >= 0.75 && out.last() != Some(&t) {
            out.push(t);
        }
        step *= 2.0;
    }
    out
}

#[cfg(test)]
mod flat_nudge_tests {
    use super::flat_nudge_seeks;

    #[test]
    fn exponential_steps_from_base() {
        let times = flat_nudge_seeks(10.0, 120.0);
        assert_eq!(times, [11.0, 12.0, 14.0, 18.0, 26.0, 42.0, 74.0]);
    }

    #[test]
    fn nudge_offsets_stay_inside_cap() {
        let times = flat_nudge_seeks(10.0, 60.0);
        assert!(times.iter().all(|&t| (0.0..60.0).contains(&t)));
        assert!(times.windows(2).all(|w| w[1] > w[0]));
        assert!(times.iter().any(|&t| t > 10.0));
        assert!(*times.last().unwrap() < 59.0);
    }

    #[test]
    fn nudge_near_end_skips_useless_clones() {
        let times = flat_nudge_seeks(58.0, 60.0);
        assert!(times.iter().all(|&t| (t - 58.0).abs() >= 0.75));
        assert!(times.iter().all(|&t| t <= 59.5));
        assert_eq!(times.len(), 1);
    }
}
