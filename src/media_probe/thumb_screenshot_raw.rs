use std::ffi::{CStr, CString};

use zenwebp::PixelLayout;

use crate::thumb_texture;

/// One encoded screenshot; `dark` marks a mostly-black frame (real dark scene or not-yet-decoded VO buffer).
struct Capture {
    webp: Vec<u8>,
    dark: bool,
}

/// Consecutive dark polls (50 ms apart) before a dark frame counts as the real decoded picture
/// (legit dark scene at the continue position) rather than an undecoded buffer.
const DARK_STABLE_POLLS: u32 = 20;

/// Poll until one decoded frame is available, then return WebP bytes (no temp files).
/// A bright frame returns immediately; a stable dark frame is accepted after
/// [DARK_STABLE_POLLS] so dark scenes (night shots, logos) still get a thumbnail.
pub(super) fn capture_screenshot_webp(m: &mut Mpv, wait_secs: u64) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(wait_secs);
    let mut polls: u32 = 0;
    let mut dark_run: u32 = 0;
    let mut dark_webp: Option<Vec<u8>> = None;
    loop {
        while m.wait_event(0.0).is_some() {}
        match try_screenshot_raw_webp(m, polls == 0) {
            Some(c) if !c.dark => return Some(c.webp),
            Some(c) => {
                dark_run += 1;
                dark_webp = Some(c.webp);
            }
            None => dark_run = 0,
        }
        if dark_run >= DARK_STABLE_POLLS {
            eprintln!("[rhino] grid_thumb dark frame accepted after {dark_run} stable polls");
            return dark_webp;
        }
        polls += 1;
        if Instant::now() >= deadline {
            if dark_webp.is_some() {
                eprintln!("[rhino] grid_thumb dark frame accepted at timeout");
                return dark_webp;
            }
            eprintln!("[rhino] grid_thumb screenshot-raw capture timeout after {polls} polls");
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
    let dark = packed_frame_mostly_black(w, h, row_stride, pf.bpp, fmt, data);
    if dark && log_blank {
        eprintln!("[rhino] grid_thumb screenshot-raw dark frame {w}x{h} fmt={fmt} (accept when stable)");
    }
    let stride_pixels = row_stride / pf.bpp;
    let webp =
        thumb_texture::encode_packed_webp(data, w as u32, h as u32, stride_pixels, pf.layout)?;
    Some(Capture { webp, dark })
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
        assert!(thumb_texture::thumb_webp_valid(&c.webp));
    }
}
