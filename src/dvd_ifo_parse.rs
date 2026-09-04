//! DVD IFO parsing (VIDEO_TS / VTS_xx_0) without linking libdvdread.

mod bitreader;
mod buf;
mod pgc;
mod streams;
mod sub_mpv_id;
mod time;
mod title_vob_durations;
mod vts;
mod vts_ptt;

use std::path::Path;

pub use streams::{
    audio_slot_for_meta, match_audio_label, match_sub_label, streams_from_vob, sub_slot_for_src_id,
    DvdIfoStreams, MpvTrackMeta,
};
pub use sub_mpv_id::{mpv_sub_id_for_ifo_slot, MpvSubTrackMeta};
pub use title_vob_durations::{
    first_substantial_vob, title_playback_sec, title_set_playback_sec, title_ttn_playback_sec,
    title_vob_durations, MIN_SUBSTANTIAL_SEC,
};
pub use vts::{chapter_marks_from_vob, IfoChapterMarks};

/// Title-set audio/sub lists from any chapter VOB path (reads `VTS_xx_0.IFO`).
pub fn ifo_streams_for_vob(vob: &Path) -> Option<DvdIfoStreams> {
    streams_from_vob(vob)
}

pub(crate) const BLOCK: usize = 2048;
pub(crate) const VTS_PTT_OFF: usize = 200;
pub(crate) const VTS_PGCIT_OFF: usize = 204;
pub(super) const PGC_SIZE: usize = 236;
pub(super) const CELL_PB_SIZE: usize = 24;
pub(super) const CELL_POS_SIZE: usize = 4;
pub(super) const MAX_MARKS: usize = 99;

const TITLE_INFO_SIZE: usize = 12;
const TT_SRPT_OFF: usize = 196;

include!("dvd_ifo_parse/main_title.rs");

/// Whole-title seconds before the main feature (interactive menus / splash).
/// Always IFO/title timeline seconds (e.g. ~1062 on Fritt), never mpv virtual tail (~89596).
#[must_use]
pub fn movie_entry_global_sec(disc: &Path) -> Option<f64> {
    let (vts_dir, vts_id, feature_ttn) = feature_title_context(disc)?;
    if let Some(skip) = skip_before_feature_ttn(
        &vts_dir.join(format!("VTS_{vts_id:02}_0.IFO")),
        feature_ttn,
    ) {
        return Some(skip);
    }
    let first_vob = crate::dvd_entity::first_chapter_vob(&vts_dir, vts_id)?;
    entry_sec_from_first_vob(&first_vob)
}

/// Video-ts directory plus the feature title's set id and title-table number.
fn feature_title_context(disc: &Path) -> Option<(std::path::PathBuf, u32, u32)> {
    let vts_dir = crate::video_ext::dvd_video_ts_dir(disc)?;
    let (vts_id, feature_ttn) = main_title_from_disc(disc)?;
    Some((vts_dir, vts_id, feature_ttn))
}

/// Entry seconds from the feature's first VOB: substantial first segment, else first mark.
fn entry_sec_from_first_vob(first_vob: &Path) -> Option<f64> {
    let durs = title_vob_durations(first_vob)?;
    if let Some(first_seg) = first_substantial_share(&durs) {
        return Some(first_seg);
    }
    let marks = chapter_marks_from_vob(first_vob)?;
    first_mark_share(&marks)
}

/// Whole-title seconds of every pre-feature title-table entry, when substantial.
fn skip_before_feature_ttn(ifo: &Path, feature_ttn: u32) -> Option<f64> {
    if feature_ttn <= 1 {
        return None;
    }
    let mut skip = 0.0_f64;
    for ttn in 1..feature_ttn {
        skip += title_ttn_playback_sec(ifo, ttn as usize).unwrap_or(0.0);
    }
    (skip >= MIN_SUBSTANTIAL_SEC).then_some(skip)
}

/// First on-disk segment when it is a substantial minority share of the title total.
fn first_substantial_share(durs: &[f64]) -> Option<f64> {
    let total: f64 = durs.iter().sum();
    durs.first().copied().filter(|&first_seg| {
        durs.len() > 1 && first_seg >= MIN_SUBSTANTIAL_SEC && first_seg <= total * 0.35
    })
}

/// First IFO chapter mark when it is a substantial minority share of the title length.
fn first_mark_share(marks: &IfoChapterMarks) -> Option<f64> {
    let first = marks.mark_secs.first().copied()?;
    (first >= MIN_SUBSTANTIAL_SEC && first <= marks.title_sec * 0.35).then_some(first)
}

pub(super) fn vts_id_from_path(path: &Path) -> Option<u32> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_uppercase)
        .and_then(|stem| {
            stem.strip_prefix("VTS_")
                .and_then(|rest| rest.split('_').next())
                .and_then(|n| n.parse().ok())
        })
}

#[cfg(test)]
#[path = "dvd_ifo_parse_tests.rs"]
mod tests;
