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
/// Offsets of the 8x8-grid sample pixels that fit inside `data` (shared by the dark/flat samplers).
fn packed_sample_offsets(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    last_channel: usize,
    data: &[u8],
) -> impl Iterator<Item = usize> + '_ {
    let step_y = (h / 8).max(1);
    let step_x = (w / 8).max(1);
    (0..h)
        .step_by(step_y)
        .flat_map(move |y| {
            let row = y * row_stride;
            (0..w).step_by(step_x).map(move |x| row + x * bpp)
        })
        .filter(move |&i| i + last_channel < data.len())
}

fn packed_frame_mostly_black(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &str,
    data: &[u8],
) -> bool {
    let (ri, gi, bi) = channel_order(fmt);
    let mut samples = 0u32;
    let mut bright = 0u32;
    for i in packed_sample_offsets(w, h, row_stride, bpp, bi, data) {
        samples += 1;
        if data[i + ri].max(data[i + gi]).max(data[i + bi]) > 12 {
            bright += 1;
        }
    }
    samples > 0 && bright * 20 < samples
}

/// Almost no color variation across samples: mpv vo=null placeholder after hr-seek, not a real picture.
fn packed_frame_mostly_flat(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &str,
    data: &[u8],
) -> bool {
    let (ri, gi, bi) = channel_order(fmt);
    let mut buckets = std::collections::HashSet::new();
    for i in packed_sample_offsets(w, h, row_stride, bpp, bi, data) {
        buckets.insert((data[i + ri] / 16, data[i + gi] / 16, data[i + bi] / 16));
    }
    buckets.len() < 8
}
