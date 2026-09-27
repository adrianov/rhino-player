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

fn rect_fits(rect: CropRect, space: (i64, i64)) -> bool {
    rect.w > 0
        && rect.h > 0
        && rect.x >= 0
        && rect.y >= 0
        && rect.x + rect.w <= space.0
        && rect.y + rect.h <= space.1
}

fn pair_dims(mpv: &Mpv, wk: &str, hk: &str) -> Option<(i64, i64)> {
    let w = mpv.get_property::<i64>(wk).ok()?;
    let h = mpv.get_property::<i64>(hk).ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

/// VO image size that validates `video-crop` (chain output, else display size).
fn vo_image_size(mpv: &Mpv) -> Option<(i64, i64)> {
    pair_dims(mpv, "video-out-params/w", "video-out-params/h")
        .or_else(|| pair_dims(mpv, "dwidth", "dheight"))
}

/// Space the cached rect was measured in: prefer `video-params` when it contains
/// the rect, else coded `width`×`height` (Smooth can make `video-params` report
/// the scaled chain size, which no longer contains a decode-space crop).
fn crop_from_space(mpv: &Mpv, rect: CropRect) -> Option<(i64, i64)> {
    if let Some(decode) = pair_dims(mpv, "video-params/w", "video-params/h") {
        if rect_fits(rect, decode) {
            return Some(decode);
        }
    }
    if let Some(coded) = pair_dims(mpv, "width", "height") {
        if rect_fits(rect, coded) {
            return Some(coded);
        }
    }
    None
}

/// Scale the canonical decode-space crop into the image the chain feeds the VO
/// now, so mpv accepts it (`video-crop` is validated against that image).
fn crop_rect_in_vo_space(mpv: &Mpv, rect: CropRect) -> CropRect {
    let Some(vo) = vo_image_size(mpv) else {
        eprintln!("[rhino] bars: video-crop map skipped (no VO size)");
        return rect;
    };
    if rect_fits(rect, vo) {
        return rect;
    }
    let Some(from) = crop_from_space(mpv, rect) else {
        eprintln!(
            "[rhino] bars: video-crop map failed (rect {} does not fit VO {}×{})",
            rect.as_video_crop(),
            vo.0,
            vo.1
        );
        return rect;
    };
    scale_rect_between(rect, from, vo)
}
