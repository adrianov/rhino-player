// Neighbour-strip paint key: skip [fill_row] only when paths and stored progress match.

use std::cell::RefCell;
use std::path::PathBuf;

pub(super) type HitsPaint = Option<Vec<(PathBuf, u64, u64)>>;

/// `false` when the strip already shows these neighbour paths at the same stored progress.
pub(super) fn take_if_new(painted: &RefCell<HitsPaint>, paths: &[PathBuf]) -> bool {
    let snap = hits_paint_snap(paths);
    if painted.borrow().as_ref() == Some(&snap) {
        return false;
    }
    *painted.borrow_mut() = Some(snap);
    true
}

fn hits_paint_snap(paths: &[PathBuf]) -> Vec<(PathBuf, u64, u64)> {
    let tpos = crate::db::load_time_pos_map();
    let durs = crate::db::load_duration_map();
    paths
        .iter()
        .map(|p| {
            let (resume, dur) = crate::playback_entity::card_resume_duration(p, &durs, &tpos);
            (p.clone(), resume.to_bits(), dur.to_bits())
        })
        .collect()
}
