// Predicts bundled vf_out WxH from rhino_60_mvtools.vpy before Super/FlowFPS.
// ME_GEOMETRY_ALIGN_PX stays in lockstep with _ME_GEOMETRY_ALIGN_PX in data/vs/rhino_60_mvtools.vpy.
// Kept in sync with bundled ME math for tests and future tuning; CPU budget changes always **`vf clr`/`vf add`**
// so a warm VapourSynth session never runs with a stale **`smooth_cap`**.

pub(crate) const ME_GEOMETRY_ALIGN_PX: i32 = 8;

fn positive_dim_u32(v: i32) -> Option<u32> {
    u32::try_from(v).ok().filter(|&u| u >= 1)
}

#[must_use]
pub(crate) fn bundled_me_vf_out_wh(
    decode_w: i32,
    decode_h: i32,
    smooth_max_area_px: u64,
) -> Option<(u32, u32)> {
    let dw = decode_w.max(1);
    let dh = decode_h.max(1);
    let smooth_cap = smooth_max_area_px.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let decode_px_u = (dw as u64).checked_mul(dh as u64)?;

    if decode_px_u <= smooth_cap {
        me_wh_unscaled(dw, dh)
    } else {
        me_wh_scaled_to_cap(dw, dh, decode_px_u, smooth_cap)
    }
}

#[inline]
fn me_wh_aligned(width_px: i32, height_px: i32) -> (i32, i32) {
    let b = ME_GEOMETRY_ALIGN_PX;
    let wf = width_px - (width_px % b);
    let hf = height_px - (height_px % b);
    (wf.max(b), hf.max(b))
}

/// Decode already fits the cap: keep it (aligned only when alignment changes dims).
fn me_wh_unscaled(dw: i32, dh: i32) -> Option<(u32, u32)> {
    let (aw, ah) = me_wh_aligned(dw, dh);
    let (ow, oh) = if aw == dw && ah == dh {
        (dw, dh)
    } else {
        (aw, ah)
    };
    Some((positive_dim_u32(ow)?, positive_dim_u32(oh)?))
}

/// Decode exceeds the cap: scale by `sqrt(cap / decode_px)`, round, floor at 2 px, align to 8.
fn me_wh_scaled_to_cap(dw: i32, dh: i32, decode_px_u: u64, smooth_cap: u64) -> Option<(u32, u32)> {
    let (me_w, me_h) = me_dims_scaled_to_cap(dw, dh, decode_px_u, smooth_cap);
    let (aw, ah) = me_wh_aligned(me_w, me_h);
    Some((positive_dim_u32(aw.max(2))?, positive_dim_u32(ah.max(2))?))
}

/// `sqrt(cap / decode_px)` scale factor, applied per axis with rounding and a 2 px floor.
fn me_dims_scaled_to_cap(dw: i32, dh: i32, decode_px_u: u64, smooth_cap: u64) -> (i32, i32) {
    let cap = smooth_cap.max(1) as f64;
    let dp = decode_px_u.max(1) as f64;
    let scale = (cap / dp).sqrt();
    let me_w = ((f64::from(dw)) * scale).round() as i32;
    let me_h = ((f64::from(dh)) * scale).round() as i32;
    (me_w.max(2), me_h.max(2))
}

#[cfg(test)]
mod smooth_me_geometry_tests {
    use super::*;

    #[test]
    fn wide_decode_scaled_dims_match_bundled_script() {
        // 3840×1600 decode; sqrt(cap/decode_px) → round → align(8), matches bundled `.vpy` style.
        let (w, h) = bundled_me_vf_out_wh(3840, 1600, 970_173).expect("dims");
        assert_eq!((w, h), (1520, 632));
    }

    #[test]
    fn two_nearby_caps_can_share_vf_out_after_align() {
        let decode = (3840_i32, 1600_i32);
        // Same ME WxH for adjacent px² budgets once round+8px-align collapses the difference.
        let a = bundled_me_vf_out_wh(decode.0, decode.1, 497_953).unwrap();
        let b = bundled_me_vf_out_wh(decode.0, decode.1, 497_954).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, (1088, 456));
    }
}
