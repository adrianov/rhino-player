use std::ffi::{CStr, CString};

use zenwebp::PixelLayout;

use crate::thumb_texture;

thread_local! {
    /// Source path for the in-flight grid-thumb capture (worker thread).
    static THUMB_SRC: RefCell<String> = const { RefCell::new(String::new()) };
}

fn thumb_src_set(path: &Path) {
    THUMB_SRC.with(|s| *s.borrow_mut() = path.display().to_string());
}

fn thumb_src_clear() {
    THUMB_SRC.with(|s| s.borrow_mut().clear());
}

fn thumb_src_suffix() -> String {
    THUMB_SRC.with(|s| {
        let s = s.borrow();
        if s.is_empty() {
            String::new()
        } else {
            format!(" {s}")
        }
    })
}

/// Clears [THUMB_SRC] when the capture scope ends (including early `?` returns).
struct ThumbSrcGuard;
impl Drop for ThumbSrcGuard {
    fn drop(&mut self) {
        thumb_src_clear();
    }
}

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
    let view = PackedView {
        w,
        h,
        row_stride,
        bpp: pf.bpp,
        fmt,
        data,
    };
    encode_thumb_capture(&view, pf.layout, log_blank)
}

fn encode_thumb_capture(
    view: &PackedView<'_>,
    layout: PixelLayout,
    log_blank: bool,
) -> Option<Capture> {
    // Skip bar crop on a dark full frame (real dark scene / empty VO).
    let crop = thumb_encode_crop(view, packed_view_mostly_black(view), log_blank);
    // Stability flags use the encoded region so side pillars do not skew flat/dark.
    let dark = packed_region_mostly_black(view, crop);
    let flat = packed_region_mostly_flat(view, crop);
    log_blank_frame(crop.w, crop.h, view.fmt, dark, flat, log_blank);
    let start = crop.y * view.row_stride + crop.x * view.bpp;
    let webp = thumb_texture::encode_packed_webp(
        &view.data[start..],
        crop.w as u32,
        crop.h as u32,
        view.row_stride / view.bpp,
        layout,
    )?;
    Some(Capture { webp, dark, flat })
}

fn log_blank_frame(w: usize, h: usize, fmt: &str, dark: bool, flat: bool, log_blank: bool) {
    if !log_blank {
        return;
    }
    if dark {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw dark frame {w}x{h} fmt={fmt} (accept when stable){}",
            thumb_src_suffix()
        );
    }
    if flat {
        eprintln!(
            "[rhino] grid_thumb screenshot-raw flat frame {w}x{h} fmt={fmt} (accept when stable){}",
            thumb_src_suffix()
        );
    }
}

/// Crop box for encode: strip baked-in bars unless the whole frame is a dark scene.
fn thumb_encode_crop(view: &PackedView<'_>, dark: bool, log_crop: bool) -> SampleRect {
    if dark {
        return SampleRect {
            x: 0,
            y: 0,
            w: view.w,
            h: view.h,
        };
    }
    match crate::black_bars::detect_packed_crop(
        view.w,
        view.h,
        view.row_stride,
        view.bpp,
        view.fmt,
        view.data,
    ) {
        Some(c) => {
            if log_crop {
                eprintln!(
                    "[rhino] grid_thumb: crop bars {}x{} -> {}x{}+{}+{}{}",
                    view.w,
                    view.h,
                    c.w,
                    c.h,
                    c.x,
                    c.y,
                    thumb_src_suffix()
                );
            }
            SampleRect {
                x: c.x as usize,
                y: c.y as usize,
                w: c.w as usize,
                h: c.h as usize,
            }
        }
        None => SampleRect {
            x: 0,
            y: 0,
            w: view.w,
            h: view.h,
        },
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
        let c = raw_frame_to_webp(w, h, (w * 4) as isize, "bgr0", &bgr0_test_pixels(w, h), true).unwrap();
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
        assert!(c.flat);
        assert!(thumb_texture::thumb_webp_valid(&c.webp));
    }

    fn write_detail_px(data: &mut [u8], i: usize, x: usize, y: usize) {
        data[i] = (x.wrapping_mul(17).wrapping_add(y.wrapping_mul(3))) as u8;
        data[i + 1] = (x.wrapping_mul(9).wrapping_add(y.wrapping_mul(11))) as u8;
        data[i + 2] = (x.wrapping_mul(5).wrapping_add(y.wrapping_mul(23))) as u8;
        data[i + 3] = 255;
    }

    fn pillarboxed_bgr0(w: usize, h: usize, bar: usize) -> Vec<u8> {
        let mut data = vec![0u8; w * h * 4];
        for n in 0..(w * h) {
            let x = n % w;
            if x < bar || x >= w - bar {
                continue;
            }
            write_detail_px(&mut data, n * 4, x, n / w);
        }
        data
    }

    /// Side pillars must not make a detailed center look flat (stability samples the crop).
    #[test]
    fn pillarboxed_detail_not_flat() {
        let (w, h) = (64usize, 32usize);
        let c = raw_frame_to_webp(
            w,
            h,
            (w * 4) as isize,
            "bgr0",
            &pillarboxed_bgr0(w, h, 8),
            false,
        )
        .unwrap();
        assert!(!c.dark && !c.flat);
        let (_, dw, dh) = zenwebp::oneshot::decode_rgb(&c.webp).unwrap();
        assert_eq!(dh, h as u32);
        assert!(dw < w as u32, "expected pillar crop, got {dw}x{dh}");
    }
}
