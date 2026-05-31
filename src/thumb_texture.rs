//! Continue-grid WebP thumbnail bytes → [gdk::Texture].

use gtk::gdk;
use zenwebp::{EncodeRequest, LossyConfig, PixelLayout};

/// Lossy quality for continue-grid WebP captures (0–100).
pub const GRID_THUMB_WEBP_Q: f32 = 82.0;

fn grid_webp_enc() -> LossyConfig {
    LossyConfig::new()
        .with_quality(GRID_THUMB_WEBP_Q)
        .with_method(4)
}

/// Encode a borrowed packed pixel buffer (`Rgb8` / `Rgba8` / `Bgr8` / `Bgra8`) to WebP.
/// No pixel copy: zenwebp reads [pixels] in place (only the WebP output is allocated).
pub fn encode_packed_webp(
    pixels: &[u8],
    width: u32,
    height: u32,
    stride_pixels: usize,
    layout: PixelLayout,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || stride_pixels < width as usize {
        eprintln!(
            "[rhino] grid_thumb webp encode bad dims {width}x{height} stride={stride_pixels}"
        );
        return None;
    }
    let config = grid_webp_enc();
    EncodeRequest::lossy(&config, pixels, layout, width, height)
        .with_stride(stride_pixels)
        .encode()
        .ok()
}

/// True when bytes look like a complete WebP still (RIFF….WEBP header).
pub fn thumb_webp_valid(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP"
}

/// Decode a stored continue-grid WebP thumbnail; returns `None` on invalid data.
pub fn texture_from_thumb(bytes: &[u8]) -> Option<gdk::Texture> {
    if !thumb_webp_valid(bytes) {
        return None;
    }
    let (rgb, w, h) = zenwebp::oneshot::decode_rgb(bytes).ok()?;
    let w = w as i32;
    let h = h as i32;
    let need = w as usize * h as usize * 3;
    if rgb.len() < need {
        eprintln!(
            "[rhino] grid_thumb webp decode short rgb={} need={need} {w}x{h}",
            rgb.len()
        );
        return None;
    }
    let stride = (w * 3) as usize;
    let tex = gdk::MemoryTexture::new(
        w,
        h,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from(&rgb[..need]),
        stride,
    );
    Some(tex.upcast())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webp_header_valid() {
        let mut w = *b"RIFFxxxxWEBP";
        w[4..8].copy_from_slice(&12u32.to_le_bytes());
        assert!(thumb_webp_valid(&w));
    }

    #[test]
    fn non_webp_bytes_rejected() {
        let j = vec![0xFF, 0xD8, 0xFF, 0xD9];
        assert!(!thumb_webp_valid(&j));
    }

    #[test]
    fn bgra_webp_roundtrip_rgb_decode() {
        let w = 4u32;
        let h = 3u32;
        let mut bgra: Vec<u8> = Vec::with_capacity(w as usize * h as usize * 4);
        for i in 0..(w * h) {
            let v = (i % 251) as u8;
            bgra.extend_from_slice(&[v.wrapping_add(2), v.wrapping_add(1), v, 255]);
        }
        let webp = encode_packed_webp(&bgra, w, h, w as usize, PixelLayout::Bgra8).expect("encode");
        assert!(thumb_webp_valid(&webp));
        let (rgb, dw, dh) = zenwebp::oneshot::decode_rgb(&webp).expect("decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(rgb.len(), w as usize * h as usize * 3);
        assert!(rgb.iter().any(|&b| b > 0));
    }
}
