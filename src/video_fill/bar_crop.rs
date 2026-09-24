//! Cross-module read access to the strip verdict: aspect fit/snap strips known
//! baked-in black strips before computing the window target ratio.

use libmpv2::Mpv;

use crate::black_bars::{BarState, CropRect};

/// Content crop known for the media mpv has open: the live strip-probe verdict
/// when final, else a fresh `media.bar_crop` row (mtime/size-checked). `None`
/// means no strips are known yet, or a finished probe / cache said clean.
pub(crate) fn known_bar_crop(mpv: &Mpv) -> Option<CropRect> {
    // A finished probe is authoritative: its Crop wins, its Clean rules strips
    // out (no cache read). Unknown / Pending → best known value from the cache.
    match super::FILL_SYNC.with(|c| c.borrow().as_ref().map(|s| s.bars.state.get())) {
        Some(BarState::Crop(rect)) => return Some(rect),
        Some(BarState::Clean) => return None,
        _ => {}
    }
    let path = crate::media_probe::local_file_from_mpv(mpv)?;
    let spec = crate::db::media_bar_crop(&path)?.crop?;
    CropRect::parse_video_crop(&spec)
}
