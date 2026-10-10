//! Picture crop when the viewport already matches the content aspect.
//!
//! The window is sized to the picture with baked-in strips removed. Without a
//! crop, mpv still scales the full frame into that window and the picture sits
//! in a margin. Crop the strips and leave panscan at 0 so the picture fills.

use super::super::FillSync;
use crate::black_bars::{apply_video_crop, clear_video_crop, CropRect};
use gtk::prelude::*;
use libmpv2::Mpv;

/// How the frame is presented for one visibility verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PictureMode {
    /// Viewport matches the picture: crop strips, no extra zoom.
    Match,
    /// User wants fill zoom (or a zoom is still applied on a mismatch).
    Zoom,
    /// Fitted full frame; baked-in strips stay visible.
    Frame,
}

pub(super) fn picture_mode(mismatch: bool, want_zoom: bool, zoom_on: bool) -> PictureMode {
    if !mismatch {
        PictureMode::Match
    } else if want_zoom || zoom_on {
        PictureMode::Zoom
    } else {
        PictureMode::Frame
    }
}

impl FillSync {
    pub(super) fn sync_view(&self, mismatch: bool) {
        match picture_mode(mismatch, self.preferred.get(), self.active.get()) {
            PictureMode::Match => self.present_matched_picture(),
            PictureMode::Zoom => self.apply_fill(self.preferred.get()),
            PictureMode::Frame => self.show_baked_strips(),
        }
        self.btn.set_visible(mismatch);
    }

    /// Viewport matches the picture. Crop baked-in strips so the image fills the
    /// window; drop fill zoom if it was still on.
    fn present_matched_picture(&self) {
        let player = self.player.borrow();
        let Some(b) = player.as_ref() else {
            return;
        };
        self.ensure_picture_crop(&b.mpv, self.bars.crop());
        if self.active.replace(false) {
            self.btn.remove_css_class("rp-fill-on");
            let _ = b.mpv.set_property("panscan", 0.0f64);
        }
    }

    fn ensure_picture_crop(&self, mpv: &Mpv, crop: Option<CropRect>) {
        // `None` while frame sizes are missing must not be stored — mpv would reject
        // the rect, and the next sync has to try again.
        let key = match crop.and_then(|rect| crop_apply_key(mpv, rect)) {
            None if crop.is_some() => return,
            key => key,
        };
        if self.applied.borrow().as_ref() == key.as_ref() {
            return;
        }
        let previous = self.applied.replace(key);
        apply_video_crop(mpv, crop);
        log_picture_crop(crop, previous.is_some());
    }

    /// Mismatch with fill off: show the full frame, including baked-in strips.
    fn show_baked_strips(&self) {
        if self.applied.borrow().is_none() {
            return;
        }
        let player = self.player.borrow();
        let Some(b) = player.as_ref() else {
            return;
        };
        self.applied.borrow_mut().take();
        clear_video_crop(&b.mpv);
        let _ = b.mpv.set_property("panscan", 0.0f64);
        eprintln!("[rhino] fill: restore full frame (viewport differs from picture)");
    }
}

/// Identity of a picture crop once both frame sizes are known. `None` while either
/// size is missing, so a Smooth downscale (or a late decode size) reapplies the
/// mapped `video-crop` instead of skipping a rejected rect.
fn crop_apply_key(mpv: &Mpv, rect: CropRect) -> Option<String> {
    let dim = |prop: &str| mpv.get_property::<i64>(prop).unwrap_or(0);
    let vw = dim("video-out-params/w");
    let vh = dim("video-out-params/h");
    let dw = dim("video-params/w");
    let dh = dim("video-params/h");
    (vw > 0 && vh > 0 && dw > 0 && dh > 0)
        .then(|| format!("{vw}x{vh}:{dw}x{dh}:{}", rect.as_video_crop()))
}

fn log_picture_crop(crop: Option<CropRect>, had: bool) {
    match crop {
        Some(rect) => eprintln!(
            "[rhino] fill: crop to picture {} (viewport matches content)",
            rect.as_video_crop()
        ),
        None if had => eprintln!("[rhino] fill: clear picture crop (viewport matches frame)"),
        None => {}
    }
}

#[cfg(test)]
mod picture_mode_tests {
    use super::{picture_mode, PictureMode};

    #[test]
    fn matched_viewport_crops_to_the_picture() {
        assert_eq!(picture_mode(false, false, false), PictureMode::Match);
        assert_eq!(picture_mode(false, true, true), PictureMode::Match);
    }

    #[test]
    fn mismatch_keeps_zoom_or_full_frame() {
        assert_eq!(picture_mode(true, true, false), PictureMode::Zoom);
        assert_eq!(picture_mode(true, false, true), PictureMode::Zoom);
        assert_eq!(picture_mode(true, false, false), PictureMode::Frame);
    }
}
