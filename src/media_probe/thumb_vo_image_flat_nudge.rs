// Flat-still nudge: if the primary capture is almost uniform, retry nearby seeks.

use libmpv2::Mpv;
use std::path::Path;

use super::vo_image_capture_after_seek;

/// Seconds offset from the primary still when that still is almost uniform (title card / solid fill).
const FLAT_NUDGE_SECS: &[f64] = &[2.5, 7.0, -2.5];

pub(super) struct FlatNudgeCtx<'a> {
    pub m: &'a mut Mpv,
    pub src: &'a Path,
    pub ifo_seek: f64,
    pub cap: f64,
    pub chain_head: bool,
    pub dvd_vob: bool,
    pub wait_secs: u64,
}

/// If `first` is almost uniform, recapture at nearby times; keep `first` only when every nudge is flat.
pub(super) fn vo_image_prefer_nonflat(ctx: FlatNudgeCtx<'_>, first: Vec<u8>) -> Option<Vec<u8>> {
    if !crate::thumb_texture::thumb_webp_is_flat_fill(&first) {
        return Some(first);
    }
    eprintln!(
        "[rhino] grid_thumb flat still at {:.2}s; trying nearby seeks {}",
        ctx.ifo_seek,
        ctx.src.display()
    );
    for nudged in flat_nudge_seeks(ctx.ifo_seek, ctx.cap) {
        let Some(b) = vo_image_capture_after_seek(
            ctx.m,
            ctx.src,
            nudged,
            ctx.chain_head,
            ctx.dvd_vob,
            ctx.wait_secs,
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
    FLAT_NUDGE_SECS
        .iter()
        .filter_map(|&d| {
            let t = crate::seek_bar_preview::cap_preview_seek_time(base + d, cap);
            ((t - base).abs() >= 0.75).then_some(t)
        })
        .collect()
}

#[cfg(test)]
mod flat_nudge_tests {
    use super::flat_nudge_seeks;

    #[test]
    fn nudge_offsets_stay_inside_cap() {
        let times = flat_nudge_seeks(10.0, 60.0);
        assert!(times.iter().all(|&t| (0.0..60.0).contains(&t)));
        assert!(times.iter().any(|&t| t > 10.0));
    }

    #[test]
    fn nudge_near_end_skips_useless_clones() {
        let times = flat_nudge_seeks(58.0, 60.0);
        assert!(times.iter().all(|&t| (t - 58.0).abs() >= 0.75 || times.is_empty()));
        assert!(times.iter().all(|&t| t <= 59.5));
    }
}
