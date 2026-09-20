// Crop-space mapping: strip rects are cached in decoded-frame space (the
// canonical, chain-independent space) while `video-crop` applies to the image
// the filter chain hands to the VO — Smooth60's size cap downscales that image,
// so probe verdicts and every set cross the two spaces. The probe captures the
// size pair beside its metadata (before teardown) and maps the verdict with it;
// `apply_video_crop` maps to the current pair so mpv accepts the property.

/// Frame sizes across the filter chain: decoded image (`video-params`) vs the
/// image the chain feeds the VO (`video-out-params`). Smooth60's size cap
/// downscales the latter; `video-crop` is validated against it, and cropdetect
/// metadata is measured on it.
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

/// Map a crop rect measured in one image space into another (chain output vs
/// decoded frame); keeps the strip fractions, clamps into the target bounds.
fn scale_rect_between(rect: CropRect, from: (i64, i64), to: (i64, i64)) -> CropRect {
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
