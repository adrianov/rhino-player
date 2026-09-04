use std::path::Path;

use super::pgc::{
    fill_ptt_marks, pgc_has_vob, title_pgc_cells, title_playback_sec, Pgc, Pgcit,
};
use super::vts_ptt::{load_vts_tables, PttTitle, VtsPtt, VtsTables};

/// PTT chapter boundaries from `VTS_xx_0.IFO` (IFO clock seconds; display only).
pub struct IfoChapterMarks {
    /// Start time of each chapter after the first, in IFO playback time.
    pub mark_secs: Vec<f64>,
    /// Whole-title length in the same IFO clock (for scaling marks onto the VOB timeline).
    pub title_sec: f64,
}

/// Read PTT chapter marks for the title set of `chapter_vob` (e.g. `VTS_02_1.VOB`).
pub fn chapter_marks_from_vob(chapter_vob: &Path) -> Option<IfoChapterMarks> {
    let disc = crate::video_ext::dvd_disc_root(chapter_vob)?;
    let vts_dir = crate::video_ext::dvd_video_ts_dir(&disc)?;
    let vts_id = super::vts_id_from_path(chapter_vob)?;
    chapter_marks_from_vts_ifo(
        &vts_dir.join(format!("VTS_{vts_id:02}_0.IFO")),
        crate::dvd_entity::vob_part_id(chapter_vob).unwrap_or(1),
    )
}

fn chapter_marks_from_vts_ifo(ifo_path: &Path, hint_vob_id: u32) -> Option<IfoChapterMarks> {
    let tables = load_vts_tables(ifo_path)?;
    let (pgc, pgc_id, start_cell, end_cell, title) = feature_pgc_cells(&tables, hint_vob_id)?;
    let title_sec = title_playback_sec(pgc, start_cell, end_cell);
    if !(title_sec.is_finite() && title_sec > 0.0) {
        return None;
    }
    let mut mark_secs = Vec::new();
    fill_ptt_marks(
        &title.ptt,
        pgc,
        pgc_id,
        start_cell,
        end_cell,
        &mut mark_secs,
    );
    Some(IfoChapterMarks {
        mark_secs,
        title_sec,
    })
}

/// Feature-title PGC span plus the raw PTT entry used to pick it.
fn feature_pgc_cells(
    tables: &VtsTables,
    hint_vob_id: u32,
) -> Option<(&Pgc, u16, usize, usize, &PttTitle)> {
    let title = tables.ptt.titles.get(pick_vts_ttn(&tables.ptt, &tables.pgcit, hint_vob_id) - 1)?;
    let (pgcn, pgn) = title.ptt.first().copied()?;
    let (pgc, pgc_id, start_cell, end_cell) = title_pgc_cells(&tables.pgcit, pgcn, pgn)?;
    Some((pgc, pgc_id, start_cell, end_cell, title))
}

fn pick_vts_ttn(ptt: &VtsPtt, pgcit: &Pgcit, hint_vob_id: u32) -> usize {
    if ptt.titles.len() <= 1 || hint_vob_id < 1 {
        return 1;
    }
    let hint = hint_vob_id as u16;
    for ttn in 1..=ptt.titles.len() {
        let title = &ptt.titles[ttn - 1];
        let Some((pgcn, pgn)) = title.ptt.first().copied() else {
            continue;
        };
        let Some((pgc, _, start, end)) = title_pgc_cells(pgcit, pgcn, pgn) else {
            continue;
        };
        if pgc_has_vob(pgc, start, end, hint) {
            return ttn;
        }
    }
    1
}
