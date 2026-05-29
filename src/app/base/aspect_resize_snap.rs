// Post-resize snap: one-axis nudge — pick smallest of +W, −W, +H, −H to match video aspect.

/// Coded video pixel size (stable across `vf`); used for aspect snap and logging.
pub(crate) type WinAspectCell = std::cell::Cell<Option<(i64, i64)>>;

const ASPECT_MIN_W: i32 = 320;
const ASPECT_MIN_H: i32 = 200;
const ASPECT_MAX_DIM: i32 = 8192;
/// Relative aspect error below this → already aligned.
const ASPECT_ALREADY_REL: f64 = 1e-5;
/// Snap when width or height is within this fraction of its aspect-correct value.
const ASPECT_DIM_TOLERANCE: f64 = 0.60;
/// Each side may grow or shrink by at most this fraction of its current value.
const ASPECT_MAX_SIDE_DELTA_FRAC: f64 = 0.50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SnapAxis {
    PlusW,
    MinusW,
    PlusH,
    MinusH,
}

pub(crate) fn win_aspect_ratio((vw, vh): (i64, i64)) -> f64 {
    vw as f64 / vh as f64
}

pub(crate) fn aspect_rel_err(ww: i32, hh: i32, vw: i64, vh: i64) -> f64 {
    let target = vw as f64 / vh as f64;
    let cur = f64::from(ww) / f64::from(hh);
    (cur - target).abs() / target
}

/// H = round(W × vh / vw)
fn height_for_width(nw: i32, vw: i64, vh: i64) -> i32 {
    ((i64::from(nw) * vh + vw / 2) / vw) as i32
}

/// W = round(H × vw / vh)
fn width_for_height(nh: i32, vw: i64, vh: i64) -> i32 {
    ((i64::from(nh) * vw + vh / 2) / vh) as i32
}

fn dim_off(cur: i32, target: i32) -> f64 {
    if target <= 0 {
        return f64::INFINITY;
    }
    f64::from((cur - target).abs()) / f64::from(target)
}

/// Width or height near its aspect-correct value.
fn snap_eligible(ww: i32, hh: i32, vw: i64, vh: i64) -> bool {
    if snap_skip_wide_pillar(ww, hh, vw, vh) {
        return false;
    }
    let target_w = width_for_height(hh, vw, vh);
    let target_h = height_for_width(ww, vw, vh);
    dim_off(ww, target_w) <= ASPECT_DIM_TOLERANCE || dim_off(hh, target_h) <= ASPECT_DIM_TOLERANCE
}

/// Short letterbox with width already near target — leave alone (e.g. 1289×540).
fn snap_skip_wide_pillar(ww: i32, hh: i32, vw: i64, vh: i64) -> bool {
    let target_w = width_for_height(hh, vw, vh);
    let target_h = height_for_width(ww, vw, vh);
    if target_w <= 0 || target_h <= 0 {
        return false;
    }
    f64::from(hh) < f64::from(target_h) * 0.75 && dim_off(ww, target_w) < 0.38
}

pub(crate) fn aspect_dim_offsets(ww: i32, hh: i32, vw: i64, vh: i64) -> (f64, f64) {
    let target_w = width_for_height(hh, vw, vh);
    let target_h = height_for_width(ww, vw, vh);
    (dim_off(ww, target_w), dim_off(hh, target_h))
}

/// Pixel deltas to match aspect on one axis: (+W grow, −W shrink, +H grow, −H shrink); 0 = not needed.
pub(crate) fn aspect_one_axis_deltas(ww: i32, hh: i32, vw: i64, vh: i64) -> (i32, i32, i32, i32) {
    let target_w = width_for_height(hh, vw, vh);
    let target_h = height_for_width(ww, vw, vh);
    let dw = target_w - ww;
    let dh = target_h - hh;
    (
        dw.max(0),
        (-dw).max(0),
        dh.max(0),
        (-dh).max(0),
    )
}

fn size_ok(nw: i32, nh: i32) -> bool {
    nw >= ASPECT_MIN_W && nh >= ASPECT_MIN_H && nw <= ASPECT_MAX_DIM && nh <= ASPECT_MAX_DIM
}

fn side_delta_ok(cur: i32, delta: i32) -> bool {
    if delta == 0 {
        return true;
    }
    f64::from(delta.unsigned_abs()) / f64::from(cur.max(1)) <= ASPECT_MAX_SIDE_DELTA_FRAC
}

fn try_axis(
    ww: i32,
    hh: i32,
    axis: SnapAxis,
    mag: i32,
    best: &mut Option<(SnapAxis, i32, i32, i32)>,
) {
    if mag == 0 {
        return;
    }
    let (nw, nh) = match axis {
        SnapAxis::PlusW => (ww + mag, hh),
        SnapAxis::MinusW => (ww - mag, hh),
        SnapAxis::PlusH => (ww, hh + mag),
        SnapAxis::MinusH => (ww, hh - mag),
    };
    if !size_ok(nw, nh) {
        return;
    }
    let (cur, delta) = match axis {
        SnapAxis::PlusW | SnapAxis::MinusW => (ww, (nw - ww).abs()),
        SnapAxis::PlusH | SnapAxis::MinusH => (hh, (nh - hh).abs()),
    };
    if !side_delta_ok(cur, delta) {
        return;
    }
    let replace = match *best {
        None => true,
        Some((_, _, _, bm)) => mag < bm,
    };
    if replace {
        *best = Some((axis, nw, nh, mag));
    }
}

/// Smallest one-axis nudge (+W / −W / +H / −H) within the 50% per-side cap.
pub(crate) fn snap_size_after_user_resize(
    ww: i32,
    hh: i32,
    vw: i64,
    vh: i64,
) -> Option<(i32, i32)> {
    if vw <= 0 || vh <= 0 || ww < 2 || hh < 2 {
        return None;
    }
    if aspect_rel_err(ww, hh, vw, vh) <= ASPECT_ALREADY_REL || !snap_eligible(ww, hh, vw, vh) {
        return None;
    }
    let (plus_w, minus_w, plus_h, minus_h) = aspect_one_axis_deltas(ww, hh, vw, vh);
    if plus_w == 0 && minus_w == 0 && plus_h == 0 && minus_h == 0 {
        return None;
    }
    let mut best: Option<(SnapAxis, i32, i32, i32)> = None;
    try_axis(ww, hh, SnapAxis::MinusW, minus_w, &mut best);
    try_axis(ww, hh, SnapAxis::PlusW, plus_w, &mut best);
    try_axis(ww, hh, SnapAxis::MinusH, minus_h, &mut best);
    try_axis(ww, hh, SnapAxis::PlusH, plus_h, &mut best);
    best.map(|(_, nw, nh, _)| (nw, nh))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VW: i64 = 1920;
    const VH: i64 = 1080;
    const VW_SD: i64 = 853;
    const VH_SD: i64 = 480;

    #[test]
    fn exact_no_change() {
        assert_eq!(snap_size_after_user_resize(960, 540, VW, VH), None);
    }

    #[test]
    fn one_px_shrink_width() {
        assert_eq!(
            snap_size_after_user_resize(961, 540, VW, VH),
            Some((960, 540))
        );
    }

    #[test]
    fn far_no_snap() {
        assert_eq!(snap_size_after_user_resize(400, 600, VW, VH), None);
    }

    #[test]
    fn wide_drag_keeps_width() {
        let s = snap_size_after_user_resize(1268, 540, VW, VH).unwrap();
        assert_eq!(s.0, 1268);
        assert_eq!(s.1, height_for_width(1268, VW, VH));
    }

    #[test]
    fn wide_drag_plus_h_small() {
        assert_eq!(
            snap_size_after_user_resize(1280, 682, VW, VH),
            Some((1280, 720))
        );
    }

    #[test]
    fn wide_too_wide_grows_height_not_shrink_width() {
        let s = snap_size_after_user_resize(1580, 720, VW, VH).unwrap();
        assert_eq!(s.0, 1580);
        assert_eq!(s.1, height_for_width(1580, VW, VH));
    }

    #[test]
    fn sd_window_shrinks_height() {
        let s = snap_size_after_user_resize(1026, 691, VW_SD, VH_SD).unwrap();
        assert_eq!(s, (1026, 577));
    }

    #[test]
    fn tall_window_shrinks_height() {
        let s = snap_size_after_user_resize(1180, 736, VW, VH).unwrap();
        assert_eq!(s, (1180, 664));
    }

    #[test]
    fn pick_smallest_delta() {
        let (plus_w, minus_w, plus_h, _minus_h) = aspect_one_axis_deltas(1268, 540, VW, VH);
        assert_eq!(plus_w, 0);
        assert_eq!(minus_w, 308);
        assert_eq!(plus_h, height_for_width(1268, VW, VH) - 540);
        assert!(plus_h < minus_w);
    }
}
