//! Continue-grid WebP thumbnail bytes → landscape [gdk::Texture] + card [gtk::Picture].
//!
//! [`grid_cover`]: cover-crop to [`GRID_CARD_ASPECT`] and build the continue-card picture
//! so natural size stays landscape (GTK `AspectFrame` alone does not cap measure).

use std::cell::RefCell;
use std::collections::HashMap;

use glib::prelude::{Cast, IsA};
use gtk::gdk;
use gtk::prelude::WidgetExt;
use zenwebp::{EncodeRequest, LossyConfig, PixelLayout};

/// Lossy quality for continue-grid WebP captures (0–100).
pub const GRID_THUMB_WEBP_Q: f32 = 82.0;

/// Continue-card / grid-thumb footprint (width ÷ height).
pub const GRID_CARD_ASPECT: f64 = 16.0 / 9.0;

/// Relative aspect error below which a decoded still is left uncropped.
const ASPECT_ALREADY: f64 = 0.02;

/// Fastest WebP encoder effort (zenwebp default is 4, which enables slower psycho paths).
const GRID_THUMB_WEBP_METHOD: u8 = 0;

fn grid_webp_enc() -> LossyConfig {
    LossyConfig::new()
        .with_quality(GRID_THUMB_WEBP_Q)
        .with_method(GRID_THUMB_WEBP_METHOD)
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
    EncodeRequest::lossy(&grid_webp_enc(), pixels, layout, width, height)
        .with_stride(stride_pixels)
        .encode()
        .ok()
}

/// True when bytes look like a complete WebP still (RIFF….WEBP header).
pub fn thumb_webp_valid(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP"
}

/// Reject almost-uniform WebP fills: solid color boards, single-hue gradients / mesh textures,
/// and mpv vo=null placeholders stored before decode finishes.
pub fn thumb_webp_is_flat_fill(bytes: &[u8]) -> bool {
    let Some((rgb, w, h)) = decode_flat_check_rgb(bytes) else {
        return true;
    };
    rgb_samples_mostly_flat(grid_rgb_samples(&rgb, w, h))
}

fn decode_flat_check_rgb(bytes: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    if !thumb_webp_valid(bytes) {
        return None;
    }
    let (rgb, w, h) = zenwebp::oneshot::decode_rgb(bytes).ok()?;
    let (w, h) = (w as usize, h as usize);
    (w > 0 && h > 0 && rgb.len() >= w * h * 3).then_some((rgb, w, h))
}

fn grid_rgb_samples(rgb: &[u8], w: usize, h: usize) -> Vec<(u8, u8, u8)> {
    let step_y = (h / 8).max(1);
    let step_x = (w / 8).max(1);
    let mut samples = Vec::with_capacity(64);
    for y in (0..h).step_by(step_y) {
        for x in (0..w).step_by(step_x) {
            push_rgb_sample(&mut samples, rgb, y * w * 3 + x * 3);
        }
    }
    samples
}

fn push_rgb_sample(out: &mut Vec<(u8, u8, u8)>, rgb: &[u8], i: usize) {
    if let Some(p) = rgb.get(i..i + 3) {
        out.push((p[0], p[1], p[2]));
    }
}

/// True when an ~8×8 sample grid is a solid fill, mono gradient, or lightly textured color board.
/// Shared by WebP flat checks and packed `screenshot-raw` frames.
pub(crate) fn rgb_samples_mostly_flat(samples: impl IntoIterator<Item = (u8, u8, u8)>) -> bool {
    let mut color = std::collections::HashSet::new();
    let mut hues = std::collections::HashSet::new();
    let mut n = 0u32;
    for (r, g, b) in samples {
        n += 1;
        color.insert((r / 16, g / 16, b / 16));
        if let Some(h) = chromatic_primary(r, g, b) {
            hues.insert(h);
        }
    }
    if n == 0 {
        return false;
    }
    // Few RGB buckets: solid / near-solid. One chromatic primary with limited bucket spread:
    // single-hue gradient or mesh (not a detailed mono-tinted scene with many shades).
    color.len() < 8 || (hues.len() < 2 && color.len() < 18)
}

/// Dominant primary for chromatic pixels; `None` for near-black or near-grey (luma-only boards).
fn chromatic_primary(r: u8, g: u8, b: u8) -> Option<u8> {
    let max = r.max(g).max(b);
    (max >= 16 && max - r.min(g).min(b) >= 16).then(|| primary_channel(r, g, b))
}

fn primary_channel(r: u8, g: u8, b: u8) -> u8 {
    if r >= g && r >= b {
        0
    } else if g >= b {
        1
    } else {
        2
    }
}

thread_local! {
    static THUMB_TEX_CACHE: RefCell<HashMap<String, (Vec<u8>, gdk::Texture)>> =
        RefCell::new(HashMap::new());
}

/// Decode WebP for [cache_key]; reuse the prior texture when blob bytes are unchanged (refill).
pub fn texture_from_thumb_cached(cache_key: &str, bytes: &[u8]) -> Option<gdk::Texture> {
    if !thumb_webp_valid(bytes) {
        return None;
    }
    THUMB_TEX_CACHE.with(|c| {
        let mut g = c.borrow_mut();
        if let Some((prev, tex)) = g.get(cache_key) {
            if prev.as_slice() == bytes {
                return Some(tex.clone());
            }
        }
        let tex = decode_thumb_texture(bytes)?;
        g.insert(cache_key.to_string(), (bytes.to_vec(), tex.clone()));
        Some(tex)
    })
}

fn decode_thumb_texture(bytes: &[u8]) -> Option<gdk::Texture> {
    let (rgb, w, h) = zenwebp::oneshot::decode_rgb(bytes).ok()?;
    let (rgb, w, h) = pack_cover_rgb(rgb, w as usize, h as usize)?;
    Some(memory_rgb_texture(&rgb, w, h))
}

fn pack_cover_rgb(rgb: Vec<u8>, w: usize, h: usize) -> Option<(Vec<u8>, usize, usize)> {
    let need = w.checked_mul(h)?.checked_mul(3)?;
    if rgb.len() < need {
        eprintln!(
            "[rhino] grid_thumb webp decode short rgb={} need={need} {w}x{h}",
            rgb.len()
        );
        return None;
    }
    Some(cover_rgb_to_aspect(rgb, w, h, GRID_CARD_ASPECT))
}

fn memory_rgb_texture(rgb: &[u8], w: usize, h: usize) -> gdk::Texture {
    let stride = w * 3;
    gdk::MemoryTexture::new(
        w as i32,
        h as i32,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from(&rgb[..w * h * 3]),
        stride,
    )
    .upcast()
}

include!("grid_cover.rs");

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
    fn solid_black_samples_are_flat() {
        let samples = [(0u8, 0, 0); 64];
        assert!(rgb_samples_mostly_flat(samples));
    }

    #[test]
    fn solid_black_webp_is_flat() {
        let w = 64u32;
        let h = 36u32;
        let mut bgra = vec![0u8; w as usize * h as usize * 4];
        for px in bgra.chunks_mut(4) {
            px[3] = 255;
        }
        let webp = encode_packed_webp(&bgra, w, h, w as usize, PixelLayout::Bgra8).expect("encode");
        assert!(thumb_webp_is_flat_fill(&webp));
    }

    #[test]
    fn flat_fill_webp_detected() {
        let w = 64u32;
        let h = 36u32;
        let mut bgra = vec![0u8; w as usize * h as usize * 4];
        for px in bgra.chunks_mut(4) {
            px[0] = 231;
            px[1] = 139;
            px[2] = 250;
            px[3] = 255;
        }
        let webp = encode_packed_webp(&bgra, w, h, w as usize, PixelLayout::Bgra8).expect("encode");
        assert!(thumb_webp_is_flat_fill(&webp));
    }

    #[test]
    fn red_luma_gradient_is_flat() {
        // Bright→dark red spans many RGB buckets but one chroma — title-card style boards.
        let samples: Vec<_> = (0..64)
            .map(|i| {
                let r = 40 + i * 3;
                (r, 8u8, 8u8)
            })
            .collect();
        assert!(rgb_samples_mostly_flat(samples));
    }

    #[test]
    fn red_mesh_luma_noise_is_flat() {
        let samples: Vec<_> = (0..64)
            .map(|i| (180u8.wrapping_add((i % 7) as u8 * 9), 12u8, 10u8))
            .collect();
        assert!(rgb_samples_mostly_flat(samples));
    }

    #[test]
    fn detailed_mono_tint_is_not_flat() {
        // One primary with rich shade variation — real picture, not a color board.
        let samples: Vec<_> = (0..64)
            .map(|i| {
                let r = 80 + (i * 2) as u8;
                let g = 20 + (i % 11) as u8 * 3;
                let b = 15 + (i % 7) as u8 * 4;
                (r, g, b)
            })
            .collect();
        assert!(!rgb_samples_mostly_flat(samples));
    }

    #[test]
    fn multi_hue_scene_is_not_flat() {
        let samples = [
            (220u8, 40, 40),
            (40, 200, 50),
            (40, 60, 220),
            (220, 200, 40),
            (200, 40, 200),
            (40, 200, 200),
            (120, 80, 40),
            (80, 40, 120),
        ];
        assert!(!rgb_samples_mostly_flat(samples));
    }

    #[test]
    fn bgra_webp_roundtrip_rgb_decode() {
        let w = 4u32;
        let h = 3u32;
        let mut bgra: Vec<u8> = Vec::with_capacity(w as usize * h as usize * 4);
        fill_bgra_ramp(&mut bgra, w * h);
        let webp = encode_packed_webp(&bgra, w, h, w as usize, PixelLayout::Bgra8).expect("encode");
        assert!(thumb_webp_valid(&webp));
        let (rgb, dw, dh) = zenwebp::oneshot::decode_rgb(&webp).expect("decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(rgb.len(), w as usize * h as usize * 3);
        assert!(rgb.iter().any(|&b| b > 0));
    }

    /// Deterministic non-uniform BGRA pixel ramp used by roundtrip tests.
    fn fill_bgra_ramp(bgra: &mut Vec<u8>, pixels: u32) {
        for i in 0..pixels {
            let v = (i % 251) as u8;
            bgra.extend_from_slice(&[v.wrapping_add(2), v.wrapping_add(1), v, 255]);
        }
    }
}
