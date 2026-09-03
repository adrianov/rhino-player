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

/// Geometry + bytes for one `screenshot-raw` packed frame.
struct PackedView<'a> {
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &'a str,
    data: &'a [u8],
}

/// Sub-rectangle inside a [PackedView] (full frame or bar-cropped picture).
#[derive(Clone, Copy)]
struct SampleRect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

/// Offsets of the 8×8-grid sample pixels inside a packed region (shared by dark/flat samplers).
fn packed_sample_offsets(
    view: &PackedView<'_>,
    r: SampleRect,
    last_channel: usize,
) -> Vec<usize> {
    let step_y = (r.h / 8).max(1);
    let step_x = (r.w / 8).max(1);
    let mut out = Vec::new();
    let mut y = 0;
    while y < r.h {
        let row = (r.y + y) * view.row_stride;
        let mut x = 0;
        while x < r.w {
            let i = row + (r.x + x) * view.bpp;
            if i + last_channel < view.data.len() {
                out.push(i);
            }
            x += step_x;
        }
        y += step_y;
    }
    out
}

/// Mostly near-black samples on the full frame (skip bar crop on dark scenes).
fn packed_view_mostly_black(view: &PackedView<'_>) -> bool {
    packed_region_mostly_black(
        view,
        SampleRect {
            x: 0,
            y: 0,
            w: view.w,
            h: view.h,
        },
    )
}

fn packed_region_mostly_black(view: &PackedView<'_>, r: SampleRect) -> bool {
    let (ri, gi, bi) = channel_order(view.fmt);
    let mut samples = 0u32;
    let mut bright = 0u32;
    for i in packed_sample_offsets(view, r, bi) {
        samples += 1;
        if view.data[i + ri]
            .max(view.data[i + gi])
            .max(view.data[i + bi])
            > 12
        {
            bright += 1;
        }
    }
    samples > 0 && bright * 20 < samples
}

/// Solid fill, single-hue gradient / mesh, or mpv vo=null placeholder — not a real picture.
fn packed_region_mostly_flat(view: &PackedView<'_>, r: SampleRect) -> bool {
    let (ri, gi, bi) = channel_order(view.fmt);
    crate::thumb_texture::rgb_samples_mostly_flat(
        packed_sample_offsets(view, r, bi).into_iter().map(|i| {
            (
                view.data[i + ri],
                view.data[i + gi],
                view.data[i + bi],
            )
        }),
    )
}
