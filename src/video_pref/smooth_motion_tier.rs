use crate::paths::{RHINO_MV_BLKSIZE_VAR, RHINO_MV_CHROMA_VAR, RHINO_MV_OVERLAP_VAR};

/// Max of mpv **`height`** (decoded), **`dheight`** (scaled to display), and last main **GLArea** draw
/// height (backing-store pixels). Drives MVTools cost for the bundled FlowFPS path.
pub(crate) fn smooth_motion_cost_height(mpv: &libmpv2::Mpv, draw_h: i32) -> i32 {
    let vid_h = mpv.get_property::<i64>("height").ok().unwrap_or(0).max(0) as i32;
    let dh = mpv.get_property::<i64>("dheight").ok().unwrap_or(0).max(0) as i32;
    let h = vid_h.max(dh).max(draw_h.max(0));
    if h <= 0 {
        1080
    } else {
        h
    }
}

/// mpv **`buffered-frames=`** for **`vf add vapoursynth`** — values are **unique per tier** so
/// [smooth_vf_matches_loaded_prefs] detects tier drift without parsing MVTools env.
pub(crate) fn smooth_vf_buffered_frames(cost_h: i32) -> i32 {
    let h = cost_h.max(1);
    if h <= 1080 {
        24
    } else if h <= 1440 {
        20
    } else if h <= 2160 {
        14
    } else {
        8
    }
}

pub(crate) fn publish_smooth_mvtools_env(cost_h: i32) {
    let h = cost_h.max(1);
    let (blk, ov, chroma) = if h <= 1080 {
        (32_i32, 16_i32, true)
    } else if h <= 1440 {
        (32, 8, true)
    } else if h <= 2160 {
        (16, 8, false)
    } else {
        (16, 4, false)
    };
    std::env::set_var(RHINO_MV_BLKSIZE_VAR, format!("{blk}"));
    std::env::set_var(RHINO_MV_OVERLAP_VAR, format!("{ov}"));
    std::env::set_var(RHINO_MV_CHROMA_VAR, if chroma { "1" } else { "0" });
}

#[cfg(test)]
mod smooth_motion_tier_tests {
    use super::*;

    #[test]
    fn buffered_frames_step_down_with_height() {
        assert_eq!(smooth_vf_buffered_frames(720), 24);
        assert_eq!(smooth_vf_buffered_frames(1080), 24);
        assert_eq!(smooth_vf_buffered_frames(1200), 20);
        assert_eq!(smooth_vf_buffered_frames(1440), 20);
        assert_eq!(smooth_vf_buffered_frames(1600), 14);
        assert_eq!(smooth_vf_buffered_frames(2160), 14);
        assert_eq!(smooth_vf_buffered_frames(2200), 8);
    }
}
