//! Apply global subtitle appearance from [crate::db::SubPrefs] to libmpv. See [docs/features/24-subtitles.md](../docs/features/24-subtitles.md).

use gtk::gdk::RGBA;

use libmpv2::Mpv;

use crate::db::SubPrefs;

/// Host-widget fill + alpha for the color control (0xRRGGBB; alpha 1.0 for text, border nearly opaque).
pub fn u32_to_rgba(u: u32) -> RGBA {
    let r = ((u >> 16) & 0xff) as f32 / 255.0;
    let g = ((u >> 8) & 0xff) as f32 / 255.0;
    let b = (u & 0xff) as f32 / 255.0;
    RGBA::new(r, g, b, 1.0)
}

pub fn rgba_to_u32(r: &RGBA) -> u32 {
    let ri = (r.red() * 255.0).round() as u32;
    let gi = (r.green() * 255.0).round() as u32;
    let bi = (r.blue() * 255.0).round() as u32;
    (ri << 16) | (gi << 8) | bi
}

/// Pushes [SubPrefs] to mpv (best-effort; some keys need `sub-ass-override` for embedded ASS).
///
/// `sub-color` / `sub-border-color` are **string** options (`#RRGGBB`); int properties are ignored
/// by libmpv and fall back to white.
pub fn apply_mpv(mpv: &Mpv, p: &SubPrefs) {
    let _ = mpv.set_property("sub-ass-override", "force");
    let c = format!("#{:06X}", p.color & 0xFFFFFF);
    let _ = mpv.set_property("sub-color", c);
    let b = format!("#{:06X}", p.border_color & 0xFFFFFF);
    let _ = mpv.set_property("sub-border-color", b);
    let _ = mpv.set_property("sub-border-size", p.border_size);
    let _ = mpv.set_property("sub-scale", p.scale);
}

/// Lifts [sub-pos] so on-screen text clears the bottom [ToolbarView] when chrome is **revealed** (0–100;
/// 100 = mpv default; lower = higher on screen). When chrome is auto-hidden, resets to 100.
pub fn apply_sub_pos_for_toolbar(mpv: &Mpv, bars_revealed: bool, bottom_h: i32, gl_h: i32) {
    let pos = if !bars_revealed {
        100i64
    } else {
        let bh = if bottom_h > 0 { bottom_h as f64 } else { 52.0 };
        let gh = gl_h.max(1) as f64;
        (100.0 - 100.0 * bh / gh).round() as i64
    }
    .clamp(0, 100);
    let _ = mpv.set_property("sub-pos", pos);
}
