// Packed-pixel format mapping and the mostly-black sampler used to flag dark `screenshot-raw` frames.

struct MpvPackedFmt {
    layout: PixelLayout,
    bpp: usize,
}

fn mpv_packed_fmt(fmt: &str) -> Option<MpvPackedFmt> {
    match fmt {
        "bgr0" | "bgr24" | "bgra" => Some(MpvPackedFmt {
            layout: if fmt == "bgr24" {
                PixelLayout::Bgr8
            } else {
                PixelLayout::Bgra8
            },
            bpp: if fmt == "bgr24" { 3 } else { 4 },
        }),
        "rgb0" | "rgb24" | "rgba" => Some(MpvPackedFmt {
            layout: if fmt == "rgb24" {
                PixelLayout::Rgb8
            } else {
                PixelLayout::Rgba8
            },
            bpp: if fmt == "rgb24" { 3 } else { 4 },
        }),
        _ => {
            eprintln!("[rhino] grid_thumb screenshot-raw unsupported format={fmt}");
            None
        }
    }
}

fn channel_order(fmt: &str) -> (usize, usize, usize) {
    match fmt {
        "bgr0" | "bgr24" | "bgra" => (2, 1, 0),
        _ => (0, 1, 2),
    }
}

/// Mostly near-black samples: a real dark scene or an undecoded / empty VO buffer.
/// The caller decides via poll stability ([DARK_STABLE_POLLS]).
fn packed_frame_mostly_black(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &str,
    data: &[u8],
) -> bool {
    let (ri, gi, bi) = channel_order(fmt);
    let step_y = (h / 8).max(1);
    let step_x = (w / 8).max(1);
    let mut samples = 0u32;
    let mut bright = 0u32;
    for y in (0..h).step_by(step_y) {
        let row = y * row_stride;
        for x in (0..w).step_by(step_x) {
            let i = row + x * bpp;
            if i + bi >= data.len() {
                continue;
            }
            samples += 1;
            let r = data[i + ri];
            let g = data[i + gi];
            let b = data[i + bi];
            if r.max(g).max(b) > 12 {
                bright += 1;
            }
        }
    }
    samples > 0 && bright * 20 < samples
}
