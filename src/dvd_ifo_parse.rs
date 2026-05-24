//! DVD IFO parsing (VIDEO_TS / VTS_xx_0) without linking libdvdread.

mod buf;
mod bitreader;
mod pgc;
mod streams;
mod sub_mpv_id;
mod time;
mod vts;

use std::path::Path;

pub use streams::{
    audio_slot_for_meta, match_audio_label, match_sub_label, streams_from_vob, sub_slot_for_src_id,
    DvdIfoStreams, MpvTrackMeta,
};
pub use sub_mpv_id::{mpv_sub_id_for_ifo_slot, MpvSubTrackMeta};
pub use vts::{chapter_marks_from_vob, IfoChapterMarks};

/// Title-set audio/sub lists from any chapter VOB path (reads `VTS_xx_0.IFO`).
pub fn ifo_streams_for_vob(vob: &Path) -> Option<DvdIfoStreams> {
    streams_from_vob(vob)
}

pub(super) const BLOCK: usize = 2048;
const TT_SRPT_OFF: usize = 196;
pub(super) const VTS_PTT_OFF: usize = 200;
pub(super) const VTS_PGCIT_OFF: usize = 204;
pub(super) const PGC_SIZE: usize = 236;
pub(super) const CELL_PB_SIZE: usize = 24;
pub(super) const CELL_POS_SIZE: usize = 4;
pub(super) const MAX_MARKS: usize = 99;

const TITLE_INFO_SIZE: usize = 12;

use buf::IfoBuf;

/// Disc-level main feature from `VIDEO_TS.IFO` (`TT_SRPT`): `(VTS number, title within VTS)`.
pub fn main_title_from_disc(disc: &Path) -> Option<(u32, u32)> {
    let vts_dir = crate::video_ext::dvd_video_ts_dir(disc)?;
    let ifo = vts_dir.join("VIDEO_TS.IFO");
    let buf = IfoBuf::load(&ifo)?;
    let sector = buf.be32(TT_SRPT_OFF) as usize;
    if sector == 0 {
        return None;
    }
    let base = sector * BLOCK;
    if base + 8 > buf.len() {
        return None;
    }
    let nr = buf.be16(base) as usize;
    if nr == 0 || nr >= 100 {
        return None;
    }
    let mut last = buf.be32(base + 4);
    if last == 0 {
        last = (nr * TITLE_INFO_SIZE + 8 - 1) as u32;
    }
    let info_len = last as usize + 1 - 8;
    let titles_off = base + 8;
    if titles_off + info_len > buf.len() || nr > info_len / TITLE_INFO_SIZE {
        return None;
    }
    let mut skip_menu = false;
    for i in 0..nr {
        let off = titles_off + i * TITLE_INFO_SIZE;
        if buf.byte(off + 6) >= 2 {
            skip_menu = true;
            break;
        }
    }
    let mut best = 0usize;
    let mut best_ptt = -1i32;
    let mut best_vts = 99u32;
    for i in 0..nr {
        let off = titles_off + i * TITLE_INFO_SIZE;
        let vts = buf.byte(off + 6) as u32;
        if skip_menu && vts < 2 {
            continue;
        }
        let ptt = buf.be16(off + 2) as i32;
        if ptt > best_ptt || (ptt == best_ptt && vts < best_vts) {
            best_ptt = ptt;
            best_vts = vts;
            best = i;
        }
    }
    let off = titles_off + best * TITLE_INFO_SIZE;
    let vts_id = buf.byte(off + 6) as u32;
    let ttn = buf.byte(off + 7).max(1) as u32;
    (1..=99).contains(&vts_id).then_some((vts_id, ttn))
}

pub(super) fn vts_id_from_path(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?.to_ascii_uppercase();
    let rest = stem.strip_prefix("VTS_")?;
    rest.split('_').next()?.parse().ok()
}

#[cfg(test)]
#[path = "dvd_ifo_parse_tests.rs"]
mod tests;
