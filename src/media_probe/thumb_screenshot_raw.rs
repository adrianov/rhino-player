use std::ffi::{CStr, CString};

use zenwebp::PixelLayout;

use crate::thumb_texture;

/// Poll until one decoded frame is available, then return WebP bytes (no temp files).
pub(super) fn capture_screenshot_webp(m: &mut Mpv, wait_secs: u64) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(wait_secs);
    loop {
        while m.wait_event(0.0).is_some() {}
        match try_screenshot_raw_webp(m) {
            Some(b) if thumb_texture::thumb_webp_valid(&b) => return Some(b),
            Some(b) => {
                eprintln!(
                    "[rhino] grid_thumb screenshot-raw incomplete webp bytes={}",
                    b.len()
                );
            }
            None => {}
        }
        if Instant::now() >= deadline {
            eprintln!("[rhino] grid_thumb screenshot-raw capture timeout");
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn try_screenshot_raw_webp(m: &Mpv) -> Option<Vec<u8>> {
    let mut root = mpv_command_ret(m, &["screenshot-raw", "video", "bgr0"])?;
    // Encode from mpv's byte slice before freeing the node (no pixel-buffer copy).
    let out = unsafe { encode_screenshot_node(&root) };
    unsafe {
        libmpv2_sys::mpv_free_node_contents(&mut root);
    }
    out
}

fn mpv_command_ret(m: &Mpv, args: &[&str]) -> Option<libmpv2_sys::mpv_node> {
    let mut cstr_args: Vec<CString> = Vec::with_capacity(args.len());
    for arg in args {
        cstr_args.push(CString::new(*arg).ok()?);
    }
    let mut ptrs: Vec<_> = cstr_args.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    let mut result = std::mem::MaybeUninit::<libmpv2_sys::mpv_node>::zeroed();
    let err = unsafe {
        libmpv2_sys::mpv_command_ret(m.ctx.as_ptr(), ptrs.as_mut_ptr(), result.as_mut_ptr())
    };
    if err < 0 {
        return None;
    }
    Some(unsafe { result.assume_init() })
}

/// Borrow mpv `screenshot-raw` pixels and hand them to zenwebp without copying.
unsafe fn encode_screenshot_node(root: &libmpv2_sys::mpv_node) -> Option<Vec<u8>> {
    let w = map_i64(root, b"w")? as usize;
    let h = map_i64(root, b"h")? as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let stride = map_i64(root, b"stride")? as isize;
    let fmt = map_format_str(root, b"format").unwrap_or("bgr0");
    let data = map_byte_slice(root, b"data")?;
    raw_frame_to_webp(w, h, stride, fmt, data)
}

unsafe fn map_i64(map: &libmpv2_sys::mpv_node, want: &[u8]) -> Option<i64> {
    let vn = map_field(map, want)?;
    if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_INT64 {
        Some(vn.u.int64)
    } else if vn.format == libmpv2_sys::mpv_format_MPV_FORMAT_DOUBLE {
        Some(vn.u.double_ as i64)
    } else {
        None
    }
}

unsafe fn map_format_str<'a>(map: &'a libmpv2_sys::mpv_node, want: &[u8]) -> Option<&'a str> {
    let vn = map_field(map, want)?;
    if vn.format != libmpv2_sys::mpv_format_MPV_FORMAT_STRING {
        return None;
    }
    let sp = vn.u.string;
    if sp.is_null() {
        return None;
    }
    CStr::from_ptr(sp).to_str().ok()
}

unsafe fn map_byte_slice<'a>(map: &'a libmpv2_sys::mpv_node, want: &[u8]) -> Option<&'a [u8]> {
    let vn = map_field(map, want)?;
    if vn.format != libmpv2_sys::mpv_format_MPV_FORMAT_BYTE_ARRAY {
        return None;
    }
    let ba = vn.u.ba;
    if ba.is_null() {
        return None;
    }
    let data = (*ba).data;
    let size = (*ba).size as usize;
    if data.is_null() || size == 0 {
        return None;
    }
    Some(std::slice::from_raw_parts(data.cast::<u8>(), size))
}

unsafe fn map_field<'a>(
    map: &'a libmpv2_sys::mpv_node,
    want: &[u8],
) -> Option<&'a libmpv2_sys::mpv_node> {
    if map.format != libmpv2_sys::mpv_format_MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list_ptr = map.u.list;
    if list_ptr.is_null() {
        return None;
    }
    let n = (*list_ptr).num as usize;
    let keys = (*list_ptr).keys;
    let vals = (*list_ptr).values;
    if keys.is_null() || vals.is_null() || n == 0 {
        return None;
    }
    for i in 0..n {
        let key_ptr = *keys.add(i);
        if key_ptr.is_null() {
            continue;
        }
        if CStr::from_ptr(key_ptr).to_bytes() != want {
            continue;
        }
        return Some(&*vals.add(i));
    }
    None
}

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

/// Reject undecoded / empty VO buffers (all near-black samples).
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

fn raw_frame_to_webp(
    w: usize,
    h: usize,
    stride: isize,
    fmt: &str,
    data: &[u8],
) -> Option<Vec<u8>> {
    let pf = mpv_packed_fmt(fmt)?;
    let row_stride = stride.unsigned_abs() as usize;
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
    if packed_frame_mostly_black(w, h, row_stride, pf.bpp, fmt, data) {
        eprintln!("[rhino] grid_thumb screenshot-raw blank frame {w}x{h} fmt={fmt}");
        return None;
    }
    let stride_pixels = row_stride / pf.bpp;
    thumb_texture::encode_packed_webp(data, w as u32, h as u32, stride_pixels, pf.layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgr0_frame_encodes_complete_webp() {
        let w = 2;
        let h = 2;
        let mut data = vec![0u8; w * h * 4];
        for (i, px) in data.chunks_mut(4).enumerate() {
            px[0] = 10 + i as u8;
            px[1] = 20 + i as u8;
            px[2] = 30 + i as u8;
            px[3] = 255;
        }
        let wbytes = raw_frame_to_webp(w, h, (w * 4) as isize, "bgr0", &data).unwrap();
        assert!(thumb_texture::thumb_webp_valid(&wbytes));
        let (rgb, dw, dh) = zenwebp::oneshot::decode_rgb(&wbytes).unwrap();
        assert_eq!((dw, dh), (w as u32, h as u32));
        assert_eq!(rgb.len(), w * h * 3);
    }

    #[test]
    fn all_black_frame_rejected() {
        let w = 8;
        let h = 8;
        let data = vec![0u8; w * h * 4];
        assert!(raw_frame_to_webp(w, h, (w * 4) as isize, "bgr0", &data).is_none());
    }
}
