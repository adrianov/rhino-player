// Main-title PGC cell list from `VTS_xx_0.IFO`.

use crate::dvd_ifo_parse::buf::IfoBuf;
use crate::dvd_ifo_parse::pgc::{
    cell_duration_sec, cell_first_sector, cell_last_sector, parse_pgcit, pgc_has_vob,
    title_pgc_cells, Pgcit,
};
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
    let buf = IfoBuf::load(ifo_path)?;
    let ptt_sec = buf.be32(VTS_PTT_OFF) as usize;
    let pgcit_sec = buf.be32(VTS_PGCIT_OFF) as usize;
    if ptt_sec == 0 || pgcit_sec == 0 {
        return None;
    }
    let ptt = parse_vts_ptt(&buf, ptt_sec)?;
    let pgcit = parse_pgcit(&buf, pgcit_sec, BLOCK)?;
    let vts_ttn = pick_vts_ttn(&ptt, &pgcit, hint_vob_id);
    let title = ptt.titles.get(vts_ttn - 1)?;
    let (pgcn, pgn) = title.ptt.first().copied()?;
    let (pgc, _, start_cell, end_cell) = title_pgc_cells(&pgcit, pgcn, pgn)?;
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
    (!out.is_empty()).then_some(out)
}

/// Sum of PGC cell playback times for one `TTN` in `VTS_xx_0.IFO`.
pub(super) fn title_ttn_playback_sec(ifo_path: &std::path::Path, ttn: usize) -> Option<f64> {
    let buf = IfoBuf::load(ifo_path)?;
    let ptt_sec = buf.be32(VTS_PTT_OFF) as usize;
    let pgcit_sec = buf.be32(VTS_PGCIT_OFF) as usize;
    if ptt_sec == 0 || pgcit_sec == 0 || ttn == 0 {
        return None;
    }
    let ptt = parse_vts_ptt(&buf, ptt_sec)?;
    let pgcit = parse_pgcit(&buf, pgcit_sec, BLOCK)?;
    let title = ptt.titles.get(ttn - 1)?;
    let (pgcn, pgn) = title.ptt.first().copied()?;
    let (pgc, _, start_cell, end_cell) = title_pgc_cells(&pgcit, pgcn, pgn)?;
    let total = crate::dvd_ifo_parse::pgc::title_playback_sec(pgc, start_cell, end_cell);
    (total > 0.0).then_some(total)
}

fn parse_vts_ptt(buf: &IfoBuf, sector: usize) -> Option<VtsPtt> {
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
        last = (nr * 4 + 8 - 1) as u32;
    }
    let info_len = last as usize + 1 - 8;
    let data_off = base + 8;
    if data_off + info_len > buf.len() {
        return None;
    }
    let mut offsets = Vec::with_capacity(nr);
    for i in 0..nr {
        let off = data_off + i * 4;
        let start = buf.be32(off);
        if start as usize + 4 > last as usize + 1 {
            return None;
        }
        offsets.push(start as usize);
    }
    let mut titles = Vec::with_capacity(nr);
    for i in 0..nr {
        let start = offsets[i];
        let n = if i + 1 < nr {
            offsets[i + 1].saturating_sub(start)
        } else {
            last as usize + 1 - start
        };
        if n % 4 != 0 {
            continue;
        }
        let nr_ptt = n / 4;
        let mut ptt = Vec::with_capacity(nr_ptt);
        for j in 0..nr_ptt {
            let o = base + start + j * 4;
            if o + 4 > buf.len() {
                break;
            }
            ptt.push((buf.be16(o), buf.be16(o + 2)));
        }
        titles.push(PttTitle { ptt });
    }
    Some(VtsPtt { titles })
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
