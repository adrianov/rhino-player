// Packed-pixel letterbox / pillarbox crop (continue-grid `screenshot-raw`).

/// Near-black channel ceiling (matches the thumb dark-frame sampler).
const BAR_LUMA_MAX: u8 = 12;
const BAR_BRIGHT_FRAC: f64 = 0.05;

struct LineProbe<'a> {
    row_stride: usize,
    bpp: usize,
    ri: usize,
    gi: usize,
    bi: usize,
    data: &'a [u8],
}

impl LineProbe<'_> {
    fn row_bar(&self, y: usize, w: usize) -> bool {
        self.line_bar(w, |x| y * self.row_stride + x * self.bpp)
    }

    fn col_bar(&self, x: usize, h: usize) -> bool {
        self.line_bar(h, |y| y * self.row_stride + x * self.bpp)
    }

    fn line_bar(&self, len: usize, index_at: impl Fn(usize) -> usize) -> bool {
        let step = (len / 16).max(1);
        let mut samples = 0u32;
        let mut bright = 0u32;
        let mut i = 0;
        while i < len {
            self.sample_bright(index_at(i), &mut samples, &mut bright);
            i += step;
        }
        samples > 0 && (bright as f64) < samples as f64 * BAR_BRIGHT_FRAC
    }

    fn sample_bright(&self, off: usize, samples: &mut u32, bright: &mut u32) {
        if off + self.bi >= self.data.len() {
            return;
        }
        *samples += 1;
        let px = &self.data[off..];
        if px[self.ri].max(px[self.gi]).max(px[self.bi]) > BAR_LUMA_MAX {
            *bright += 1;
        }
    }
}

/// Content crop that removes letterbox / pillarbox strips, or `None` when nothing to strip.
pub fn detect_packed_crop(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &str,
    data: &[u8],
) -> Option<CropRect> {
    if w < 8 || h < 8 {
        return None;
    }
    let (ri, gi, bi) = channel_order(fmt)?;
    let p = LineProbe {
        row_stride,
        bpp,
        ri,
        gi,
        bi,
        data,
    };
    let (top, ch) = vertical_content(&p, w, h);
    let (left, cw) = horizontal_content(&p, w, h);
    if !crop_meaningful(w as i64, h as i64, cw as i64, ch as i64) {
        return None;
    }
    Some(CropRect {
        x: left as i64,
        y: top as i64,
        w: cw as i64,
        h: ch as i64,
    })
}

fn channel_order(fmt: &str) -> Option<(usize, usize, usize)> {
    match fmt {
        "bgr0" | "bgr24" | "bgra" => Some((2, 1, 0)),
        "rgb0" | "rgb24" | "rgba" => Some((0, 1, 2)),
        _ => None,
    }
}

fn vertical_content(p: &LineProbe<'_>, w: usize, h: usize) -> (usize, usize) {
    let top = count_edge_bars(h, |y| p.row_bar(y, w));
    let bottom = count_edge_bars(h - top, |i| p.row_bar(h - 1 - i, w));
    (top, h - top - bottom)
}

fn horizontal_content(p: &LineProbe<'_>, w: usize, h: usize) -> (usize, usize) {
    let left = count_edge_bars(w, |x| p.col_bar(x, h));
    let right = count_edge_bars(w - left, |i| p.col_bar(w - 1 - i, h));
    (left, w - left - right)
}

fn count_edge_bars(limit: usize, mut is_bar: impl FnMut(usize) -> bool) -> usize {
    let mut n = 0;
    while n < limit && is_bar(n) {
        n += 1;
    }
    n
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    fn letterboxed_bgr0(w: usize, h: usize, bar: usize) -> Vec<u8> {
        let mut data = vec![0u8; w * h * 4];
        for y in bar..h.saturating_sub(bar) {
            for x in 0..w {
                let i = (y * w + x) * 4;
                data[i] = 40;
                data[i + 1] = 80;
                data[i + 2] = 120;
                data[i + 3] = 255;
            }
        }
        data
    }

    #[test]
    fn detects_letterbox_bars() {
        let w = 64;
        let h = 48;
        let bar = 8;
        let data = letterboxed_bgr0(w, h, bar);
        let crop = detect_packed_crop(w, h, w * 4, 4, "bgr0", &data).unwrap();
        assert_eq!(crop.y, bar as i64);
        assert_eq!(crop.h, (h - 2 * bar) as i64);
        assert_eq!(crop.x, 0);
        assert_eq!(crop.w, w as i64);
    }

    #[test]
    fn plain_frame_has_no_crop() {
        let w = 32;
        let h = 24;
        let mut data = vec![0u8; w * h * 4];
        for px in data.chunks_mut(4) {
            px[0] = 90;
            px[1] = 100;
            px[2] = 110;
            px[3] = 255;
        }
        assert!(detect_packed_crop(w, h, w * 4, 4, "bgr0", &data).is_none());
    }
}
