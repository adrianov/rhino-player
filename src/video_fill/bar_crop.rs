//! Cross-module read access to the strip verdict: aspect fit/snap strips known
//! baked-in black strips before computing the window target ratio.

use std::path::Path;

use libmpv2::Mpv;

use crate::black_bars::{crop_geometry_ok, BarState, CropRect};

/// Content crop known for the media mpv has open: the live strip-probe verdict
/// when final, else a fresh `media.bar_crop` row (mtime/size-checked). `None`
/// means no strips are known yet, or a finished probe / cache said clean.
pub(crate) fn known_bar_crop(mpv: &Mpv) -> Option<CropRect> {
    let path = crate::media_probe::local_file_from_mpv(mpv)?;
    known_bar_crop_for_path(&path).and_then(|rect| geometry_checked(mpv, rect))
}

/// Strip crop for a local path: live probe when it owns that file, else DB cache.
/// Used by the seek-bar preview aux player (no main `Mpv` required).
pub(crate) fn known_bar_crop_for_path(path: &Path) -> Option<CropRect> {
    match live_bar_state_for_path(path) {
        Some(BarState::Crop(rect)) => return Some(rect),
        Some(BarState::Clean) => return None,
        // Unknown / Pending / no live owner → fall through to the DB row.
        _ => {}
    }
    let spec = crate::db::media_bar_crop(path)?.crop?;
    CropRect::parse_video_crop(&spec)
}

/// Finished live strip state when FillSync's open media is `path`.
fn live_bar_state_for_path(path: &Path) -> Option<BarState> {
    super::FILL_SYNC.with(|c| {
        let sync = c.borrow().clone()?;
        let g = sync.player.borrow();
        let open = crate::media_probe::local_file_from_mpv(&g.as_ref()?.mpv)?;
        crate::video_ext::paths_same_file(&open, path).then(|| sync.bars.state.get())
    })
}

/// Drop implausible letterbox geometry when the coded frame size is known.
fn geometry_checked(mpv: &Mpv, rect: CropRect) -> Option<CropRect> {
    let Ok(fw) = mpv.get_property::<i64>("width") else {
        return Some(rect);
    };
    let Ok(fh) = mpv.get_property::<i64>("height") else {
        return Some(rect);
    };
    if fw <= 0 || fh <= 0 {
        return Some(rect);
    }
    match crop_geometry_ok(fw, fh, rect) {
        Ok(()) => Some(rect),
        Err(reason) => {
            eprintln!(
                "[rhino] bars: crop rejected ({reason}) {} on {fw}x{fh}",
                rect.as_video_crop()
            );
            None
        }
    }
}
