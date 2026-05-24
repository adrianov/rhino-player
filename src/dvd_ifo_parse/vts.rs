use std::path::Path;

use super::buf::IfoBuf;
use super::pgc::{
    fill_ptt_marks, parse_pgcit, pgc_has_vob, title_pgc_cells, title_playback_sec, Pgcit,
};
use super::{BLOCK, VTS_PGCIT_OFF, VTS_PTT_OFF};

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
    let hint = crate::dvd_entity::vob_part_id(chapter_vob).unwrap_or(1);
    let ifo = vts_dir.join(format!("VTS_{vts_id:02}_0.IFO"));
    chapter_marks_from_vts_ifo(&ifo, hint)
}

fn chapter_marks_from_vts_ifo(ifo_path: &Path, hint_vob_id: u32) -> Option<IfoChapterMarks> {
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
    let (pgc, pgc_id, start_cell, end_cell) = title_pgc_cells(&pgcit, pgcn, pgn)?;
    let title_sec = title_playback_sec(pgc, start_cell, end_cell);
    if !(title_sec.is_finite() && title_sec > 0.0) {
        return None;
    }
    let mut mark_secs = Vec::new();
    fill_ptt_marks(&title.ptt, pgc, pgc_id, start_cell, end_cell, &mut mark_secs);
    Some(IfoChapterMarks {
        mark_secs,
        title_sec,
    })
}

struct PttTitle {
    ptt: Vec<(u16, u16)>,
}

struct VtsPtt {
    titles: Vec<PttTitle>,
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
