// Main-title PGC cell list from `VTS_xx_0.IFO`.

use crate::dvd_ifo_parse::buf::IfoBuf;
use crate::dvd_ifo_parse::pgc::{
    cell_duration_sec, cell_first_sector, cell_last_sector, parse_pgcit, pgc_has_vob,
    title_pgc_cells, Pgc, Pgcit,
};
#[path = "title_pgc_ptt.rs"]
mod ptt;

use crate::dvd_ifo_parse::{vts_id_from_path, BLOCK, VTS_PGCIT_OFF, VTS_PTT_OFF};

/// One main-title PGC cell: playback length and sector span in the title chain.
pub struct TitleCell {
    pub dur_sec: f64,
    pub first_sector: u32,
    pub last_sector: u32,
}

struct PttTitle {
    ptt: Vec<(u16, u16)>,
}

struct VtsPtt {
    titles: Vec<PttTitle>,
}

struct VtsTables {
    ptt: VtsPtt,
    pgcit: Pgcit,
}

fn load_vts_tables(ifo_path: &std::path::Path) -> Option<VtsTables> {
    let buf = IfoBuf::load(ifo_path)?;
    let ptt_sec = buf.be32(VTS_PTT_OFF) as usize;
    let pgcit_sec = buf.be32(VTS_PGCIT_OFF) as usize;
    if ptt_sec == 0 || pgcit_sec == 0 {
        return None;
    }
    Some(VtsTables {
        pgcit: parse_pgcit(&buf, pgcit_sec, BLOCK)?,
        ptt: VtsPtt {
            titles: ptt::parse_titles(&buf, ptt_sec)?,
        },
    })
}

pub(super) fn title_cells(chapter_vob: &std::path::Path) -> Option<Vec<TitleCell>> {
    let disc = crate::video_ext::dvd_disc_root(chapter_vob)?;
    let vts_dir = crate::video_ext::dvd_video_ts_dir(&disc)?;
    let vts_id = vts_id_from_path(chapter_vob)?;
    let hint = crate::dvd_entity::vob_part_id(chapter_vob).unwrap_or(1);
    let ifo = vts_dir.join(format!("VTS_{vts_id:02}_0.IFO"));
    title_cells_from_ifo(&ifo, hint)
}

pub(super) fn title_cells_from_ifo(
    ifo_path: &std::path::Path,
    hint_vob_id: u32,
) -> Option<Vec<TitleCell>> {
    let tables = load_vts_tables(ifo_path)?;
    let (pgc, start_cell, end_cell) = main_pgc_cells(&tables, hint_vob_id)?;
    let cells = collect_title_cells(pgc, start_cell, end_cell);
    (!cells.is_empty()).then_some(cells)
}

/// PGC cell span of the feature title picked for `hint_vob_id`.
fn main_pgc_cells(tables: &VtsTables, hint_vob_id: u32) -> Option<(&Pgc, usize, usize)> {
    let vts_ttn = pick_vts_ttn(&tables.ptt, &tables.pgcit, hint_vob_id);
    let title = tables.ptt.titles.get(vts_ttn - 1)?;
    let (pgcn, pgn) = title.ptt.first().copied()?;
    let (pgc, _, start_cell, end_cell) = title_pgc_cells(&tables.pgcit, pgcn, pgn)?;
    Some((pgc, start_cell, end_cell))
}

fn collect_title_cells(pgc: &Pgc, start_cell: usize, end_cell: usize) -> Vec<TitleCell> {
    let mut out = Vec::new();
    for c in start_cell..=end_cell {
        let d = cell_duration_sec(pgc, c);
        if !(d.is_finite() && d > 0.0) {
            continue;
        }
        out.push(TitleCell {
            dur_sec: d,
            first_sector: cell_first_sector(pgc, c),
            last_sector: cell_last_sector(pgc, c),
        });
    }
    out
}

/// Sum of PGC cell playback times for one `TTN` in `VTS_xx_0.IFO`.
pub(super) fn title_ttn_playback_sec(ifo_path: &std::path::Path, ttn: usize) -> Option<f64> {
    if ttn == 0 {
        return None;
    }
    let tables = load_vts_tables(ifo_path)?;
    let title = tables.ptt.titles.get(ttn - 1)?;
    let (pgcn, pgn) = title.ptt.first().copied()?;
    let (pgc, _, start_cell, end_cell) = title_pgc_cells(&tables.pgcit, pgcn, pgn)?;
    let total = crate::dvd_ifo_parse::pgc::title_playback_sec(pgc, start_cell, end_cell);
    (total > 0.0).then_some(total)
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
