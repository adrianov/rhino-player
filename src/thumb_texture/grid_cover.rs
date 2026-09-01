// Landscape cover for continue-grid stills: RGB crop + card `GtkPicture`.
// Crop keeps `GtkPicture` natural size at [GRID_CARD_ASPECT] so cards share one
// footprint (GTK `AspectFrame` alone does not cap measure).

/// Center-crop packed RGB8 to `aspect` (width/height), cover-style.
fn cover_rgb_to_aspect(rgb: Vec<u8>, w: usize, h: usize, aspect: f64) -> (Vec<u8>, usize, usize) {
    if w == 0 || h == 0 || aspect <= 0.0 {
        return (rgb, w, h);
    }
    let have = w as f64 / h as f64;
    if (have - aspect).abs() / aspect <= ASPECT_ALREADY {
        return (rgb, w, h);
    }
    if have > aspect {
        crop_rgb_x(rgb, w, h, ((h as f64 * aspect).round() as usize).clamp(1, w))
    } else {
        crop_rgb_y(rgb, w, h, ((w as f64 / aspect).round() as usize).clamp(1, h))
    }
}

fn crop_rgb_x(rgb: Vec<u8>, w: usize, h: usize, new_w: usize) -> (Vec<u8>, usize, usize) {
    let x0 = (w - new_w) / 2;
    let mut out = Vec::with_capacity(new_w * h * 3);
    for y in 0..h {
        let start = (y * w + x0) * 3;
        out.extend_from_slice(&rgb[start..start + new_w * 3]);
    }
    (out, new_w, h)
}

fn crop_rgb_y(rgb: Vec<u8>, w: usize, h: usize, new_h: usize) -> (Vec<u8>, usize, usize) {
    let y0 = (h - new_h) / 2;
    let row = w * 3;
    let start = y0 * row;
    let end = (y0 + new_h) * row;
    (rgb[start..end].to_vec(), w, new_h)
}

/// Full-bleed cover picture for a continue card (texture already landscape-cropped).
pub(crate) fn cover_picture(tex: &impl IsA<gdk::Paintable>) -> gtk::Picture {
    let pic = gtk::Picture::for_paintable(tex);
    pic.set_content_fit(gtk::ContentFit::Cover);
    pic.set_can_shrink(true);
    pic.set_vexpand(true);
    pic.set_hexpand(true);
    pic.set_halign(gtk::Align::Fill);
    pic.set_valign(gtk::Align::Fill);
    pic.set_can_target(false);
    pic.add_css_class("rp-recent-bg");
    pic
}

/// True when [card]'s main child is a [gtk::Picture] showing exactly [tex].
pub(crate) fn overlay_shows_texture(card: &gtk::Overlay, tex: &gdk::Texture) -> bool {
    let Some(child) = card.child() else {
        return false;
    };
    let Ok(pic) = child.downcast::<gtk::Picture>() else {
        return false;
    };
    pic.paintable()
        .and_then(|p| p.downcast::<gdk::Texture>().ok())
        .is_some_and(|t| t == *tex)
}

#[cfg(test)]
mod grid_cover_tests {
    use super::*;

    #[test]
    fn cover_crop_portrait_to_landscape() {
        let (w, h) = (90usize, 160usize);
        let rgb = vec![7u8; w * h * 3];
        let (out, nw, nh) = cover_rgb_to_aspect(rgb, w, h, GRID_CARD_ASPECT);
        assert_eq!(nw, w);
        assert_eq!(nh, ((w as f64 / GRID_CARD_ASPECT).round() as usize));
        assert_eq!(out.len(), nw * nh * 3);
        assert!(nw as f64 / nh as f64 > 1.0);
    }

    #[test]
    fn cover_crop_square_to_landscape() {
        let (w, h) = (120usize, 120usize);
        let rgb = vec![9u8; w * h * 3];
        let (out, nw, nh) = cover_rgb_to_aspect(rgb, w, h, GRID_CARD_ASPECT);
        assert_eq!(nw, w);
        assert_eq!(nh, ((w as f64 / GRID_CARD_ASPECT).round() as usize));
        assert_eq!(out.len(), nw * nh * 3);
    }

    #[test]
    fn cover_crop_wide_to_landscape() {
        let (w, h) = (320usize, 120usize); // ~2.67:1
        let rgb = vec![3u8; w * h * 3];
        let (out, nw, nh) = cover_rgb_to_aspect(rgb, w, h, GRID_CARD_ASPECT);
        assert_eq!(nh, h);
        assert_eq!(nw, ((h as f64 * GRID_CARD_ASPECT).round() as usize));
        assert_eq!(out.len(), nw * nh * 3);
    }

    #[test]
    fn cover_crop_skips_near_aspect() {
        let (w, h) = (160usize, 90usize); // exact 16:9
        let rgb = vec![1u8; w * h * 3];
        let (out, nw, nh) = cover_rgb_to_aspect(rgb.clone(), w, h, GRID_CARD_ASPECT);
        assert_eq!((nw, nh), (w, h));
        assert_eq!(out, rgb);
    }
}
