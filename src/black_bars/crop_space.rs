// Crop-space mapping: strip rects are cached in decoded-frame space (the
// canonical, chain-independent space) while `video-crop` applies to the image
// the filter chain hands to the VO — Smooth60's size cap downscales that image,
// so probe verdicts and every set cross the two spaces. Each probe sample
// captures the size pair beside its frame and the verdict maps with it;
// `apply_video_crop` maps to the current pair so mpv accepts the property.

/// Frame sizes across the filter chain: decoded image (`video-params`) vs the
/// image the chain feeds the VO (`video-out-params`). Smooth60's size cap
/// downscales the latter; `video-crop` is validated against it, and probe
/// readings are measured on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChainSizes {
    decode: (i64, i64),
    vo: (i64, i64),
}

fn chain_sizes(mpv: &Mpv) -> Option<ChainSizes> {
    let dw = mpv.get_property::<i64>("video-params/w").ok()?;
    let dh = mpv.get_property::<i64>("video-params/h").ok()?;
    let ow = mpv.get_property::<i64>("video-out-params/w").ok()?;
    let oh = mpv.get_property::<i64>("video-out-params/h").ok()?;
    (dw > 0 && dh > 0 && ow > 0 && oh > 0).then_some(ChainSizes {
        decode: (dw, dh),
        vo: (ow, oh),
    })
}

/// Region of the VO image that `video-crop` currently shows (the full image when
/// uncropped). Screenshots of the displayed frame cover only this region.
fn shown_vo_region(mpv: &Mpv, vo: (i64, i64)) -> CropRect {
    let prop = |k: &str| mpv.get_property::<i64>(&format!("video-out-params/crop-{k}")).ok();
    match (prop("x"), prop("y"), prop("w"), prop("h")) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => CropRect { w, h, x, y },
        _ => CropRect { w: vo.0, h: vo.1, x: 0, y: 0 },
    }
}

fn union_rect(a: CropRect, b: CropRect) -> CropRect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    CropRect {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}

fn rect_in_frame(width: i64, height: i64, r: CropRect) -> bool {
    r.x >= 0 && r.y >= 0 && r.w > 0 && r.h > 0 && r.x + r.w <= width && r.y + r.h <= height
}

/// Baked-in strips are centered: keep only the thinner margin per axis so a
/// scene that is dark on one side is not cropped as a strip.
fn centered_rect(r: CropRect, (fw, fh): (i64, i64)) -> CropRect {
    let mx = r.x.min(fw - r.x - r.w);
    let my = r.y.min(fh - r.y - r.h);
    CropRect {
        x: mx,
        y: my,
        w: fw - 2 * mx,
        h: fh - 2 * my,
    }
}

/// Map a crop rect measured in one image space into another (chain output vs
/// decoded frame); keeps the strip fractions, clamps into the target bounds.
pub(crate) fn scale_rect_between(rect: CropRect, from: (i64, i64), to: (i64, i64)) -> CropRect {
    let (fw, fh) = from;
    let (tw, th) = to;
    let axis = |v: i64, f: i64, t: i64| (v as f64 * t as f64 / f as f64).round() as i64;
    let w = axis(rect.w, fw, tw).clamp(0, tw);
    let h = axis(rect.h, fh, th).clamp(0, th);
    CropRect {
        w,
        h,
        x: axis(rect.x, fw, tw).clamp(0, tw - w),
        y: axis(rect.y, fh, th).clamp(0, th - h),
    }
}

/// Scale the canonical decode-space crop into the image the chain feeds the VO
/// now, so mpv accepts it (`video-crop` is validated against that image).
fn crop_rect_in_vo_space(mpv: &Mpv, rect: CropRect) -> CropRect {
    match chain_sizes(mpv) {
        Some(s) if s.decode != s.vo => scale_rect_between(rect, s.decode, s.vo),
        _ => rect,
    }
}
