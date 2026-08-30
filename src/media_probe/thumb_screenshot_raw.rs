use std::ffi::{CStr, CString};

use zenwebp::PixelLayout;

use crate::thumb_texture;

/// Validated packed-frame geometry; returns the positive row stride in bytes.
fn packed_frame_dims_ok(
    w: usize,
    h: usize,
    stride: isize,
    pf: &MpvPackedFmt,
    fmt: &str,
    data: &[u8],
) -> Option<usize> {
    let row_stride = stride.unsigned_abs();
    if row_stride < w * pf.bpp {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw short stride={row_stride} need={} {w}x{h} fmt={fmt}",
            w * pf.bpp
        );
        return None;
    }
    let need = if h > 0 {
        row_stride * (h - 1) + w * pf.bpp
    } else {
        0
    };
    if data.len() < need {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw short data={} need={need} {w}x{h} fmt={fmt}",
            data.len()
        );
        return None;
    }
    Some(row_stride)
}

fn try_screenshot_raw_webp(m: &Mpv, log_blank: bool) -> Option<Capture> {
    let mut root = mpv_command_ret(m, &["screenshot-raw", "video"])?;
    // Encode from mpv's byte slice before freeing the node (no pixel-buffer copy).
    let out = unsafe { encode_screenshot_node(&root, log_blank) };
    unsafe {
        libmpv2_sys::mpv_free_node_contents(&mut root);
    }
    let c = out?;
    if !thumb_texture::thumb_webp_valid(&c.webp) {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw incomplete webp bytes={}",
            c.webp.len()
        );
        return None;
    }
    Some(c)
}

include!("thumb_mpv_node.rs");

include!("thumb_screenshot_poll.rs");
include!("thumb_frame_dark.rs");

/// Borrow mpv `screenshot-raw` pixels and hand them to zenwebp without copying.
unsafe fn encode_screenshot_node(root: &libmpv2_sys::mpv_node, log_blank: bool) -> Option<Capture> {
    let w = map_i64(root, b"w")? as usize;
    let h = map_i64(root, b"h")? as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let stride = map_i64(root, b"stride")? as isize;
    let fmt = map_format_str(root, b"format").unwrap_or("bgr0");
    let data = map_byte_slice(root, b"data")?;
    raw_frame_to_webp(w, h, stride, fmt, data, log_blank)
}

fn raw_frame_to_webp(
    w: usize,
    h: usize,
    stride: isize,
    fmt: &str,
    data: &[u8],
    log_blank: bool,
) -> Option<Capture> {
    let pf = mpv_packed_fmt(fmt)?;
    let row_stride = packed_frame_dims_ok(w, h, stride, &pf, fmt, data)?;
    let dark = packed_frame_mostly_black(w, h, row_stride, pf.bpp, fmt, data);
    let flat = !dark && packed_frame_mostly_flat(w, h, row_stride, pf.bpp, fmt, data);
    log_blank_frame(w, h, fmt, dark, flat, log_blank);
    let webp = encode_thumb_region(w, h, row_stride, &pf, fmt, data, dark)?;
    Some(Capture { webp, dark, flat })
}

fn encode_thumb_region(
    w: usize,
    h: usize,
    row_stride: usize,
    pf: &MpvPackedFmt,
    fmt: &str,
    data: &[u8],
    dark: bool,
) -> Option<Vec<u8>> {
    let (ox, oy, cw, ch) = thumb_encode_crop(w, h, row_stride, pf.bpp, fmt, data, dark);
    let start = oy * row_stride + ox * pf.bpp;
    thumb_texture::encode_packed_webp(
        &data[start..],
        cw as u32,
        ch as u32,
        row_stride / pf.bpp,
        pf.layout,
    )
}

fn log_blank_frame(w: usize, h: usize, fmt: &str, dark: bool, flat: bool, log_blank: bool) {
    if !log_blank {
        return;
    }
    if dark {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw dark frame {w}x{h} fmt={fmt} (accept when stable)"
        );
    }
    if flat {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw flat frame {w}x{h} fmt={fmt} (accept when stable)"
        );
    }
}

/// Crop box for encode: strip baked-in bars unless the whole frame is a dark scene.
fn thumb_encode_crop(
    w: usize,
    h: usize,
    row_stride: usize,
    bpp: usize,
    fmt: &str,
    data: &[u8],
    dark: bool,
) -> (usize, usize, usize, usize) {
    if dark {
        return (0, 0, w, h);
    }
    match crate::black_bars::detect_packed_crop(w, h, row_stride, bpp, fmt, data) {
        Some(c) => {
            eprintln!(
                "[rhino] grid_thumb: crop bars {w}x{h} -> {}x{}+{}+{}",
                c.w, c.h, c.x, c.y
            );
            (c.x as usize, c.y as usize, c.w as usize, c.h as usize)
        }
        None => (0, 0, w, h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bgr0_test_pixels(w: usize, h: usize) -> Vec<u8> {
        let mut data = vec![0u8; w * h * 4];
        for (i, px) in data.chunks_mut(4).enumerate() {
            px[0] = 10 + i as u8;
            px[1] = 20 + i as u8;
            px[2] = 30 + i as u8;
            px[3] = 255;
        }
        data
    }

    #[test]
    fn bgr0_frame_encodes_complete_webp() {
        let w = 2;
        let h = 2;
        let data = bgr0_test_pixels(w, h);
        let c = raw_frame_to_webp(w, h, (w * 4) as isize, "bgr0", &data, true).unwrap();
        assert!(!c.dark);
        assert!(thumb_texture::thumb_webp_valid(&c.webp));
        let (rgb, dw, dh) = zenwebp::oneshot::decode_rgb(&c.webp).unwrap();
        assert_eq!((dw, dh), (w as u32, h as u32));
        assert_eq!(rgb.len(), w * h * 3);
    }

    #[test]
    fn all_black_frame_marked_dark() {
        let w = 8;
        let h = 8;
        let data = vec![0u8; w * h * 4];
        let c = raw_frame_to_webp(w, h, (w * 4) as isize, "bgr0", &data, true).unwrap();
        assert!(c.dark);
        assert!(!c.flat);
        assert!(thumb_texture::thumb_webp_valid(&c.webp));
    }
}
