use super::buf::IfoBuf;
use super::time::dvdtime_to_sec;
use super::{CELL_PB_SIZE, CELL_POS_SIZE, MAX_MARKS, PGC_SIZE};

pub(super) struct Pgc {
    pub(super) nr_cells: u8,
    pub(super) program_map: Vec<u8>,
    cell_playback: Vec<[u8; CELL_PB_SIZE]>,
    cell_position: Vec<[u8; CELL_POS_SIZE]>,
}

pub(super) struct Pgcit {
    pub(super) pgcs: Vec<(u32, Pgc)>,
}

pub(super) fn parse_pgcit(buf: &IfoBuf, sector: usize, block: usize) -> Option<Pgcit> {
    let base = sector * block;
    if base + 8 > buf.len() {
        return None;
    }
    let nr = buf.be16(base) as usize;
    if nr == 0 || nr >= 10_000 {
        return None;
    }
    let last = buf.be32(base + 4) as usize;
    let srp_off = base + 8;
    if srp_off + nr * 8 > buf.len() {
        return None;
    }
    let mut pgcs = Vec::new();
    for i in 0..nr {
        let o = srp_off + i * 8;
        let start = buf.be32(o + 4);
        if start as usize + PGC_SIZE > last + 1 {
            continue;
        }
        if let Some(pgc) = read_pgc(buf, base + start as usize) {
            pgcs.push((start, pgc));
        }
    }
    (!pgcs.is_empty()).then_some(Pgcit { pgcs })
}

fn read_pgc(buf: &IfoBuf, off: usize) -> Option<Pgc> {
    let raw = buf.slice(off, PGC_SIZE)?;
    let nr_programs = raw[2];
    let nr_cells = raw[3];
    if nr_programs == 0 || nr_cells == 0 {
        return None;
    }
    let pm_off = u16::from_be_bytes([raw[230], raw[231]]) as usize;
    let cpb_off = u16::from_be_bytes([raw[232], raw[233]]) as usize;
    let cpos_off = u16::from_be_bytes([raw[234], raw[235]]) as usize;
    if pm_off == 0 || cpb_off == 0 || cpos_off == 0 {
        return None;
    }
    let pm_base = off + pm_off;
    let mut program_map = Vec::with_capacity(nr_programs as usize);
    for i in 0..nr_programs as usize {
        program_map.push(buf.byte(pm_base + i));
    }
    let cpb_base = off + cpb_off;
    let mut cell_playback = Vec::with_capacity(nr_cells as usize);
    for i in 0..nr_cells as usize {
        let cell = buf.slice(cpb_base + i * CELL_PB_SIZE, CELL_PB_SIZE)?;
        let mut c = [0u8; CELL_PB_SIZE];
        c.copy_from_slice(cell);
        cell_playback.push(c);
    }
    let cpos_base = off + cpos_off;
    let mut cell_position = Vec::with_capacity(nr_cells as usize);
    for i in 0..nr_cells as usize {
        let cell = buf.slice(cpos_base + i * CELL_POS_SIZE, CELL_POS_SIZE)?;
        let mut c = [0u8; CELL_POS_SIZE];
        c.copy_from_slice(cell);
        cell_position.push(c);
    }
    Some(Pgc {
        nr_cells,
        program_map,
        cell_playback,
        cell_position,
    })
}

pub(super) fn title_pgc_cells(
    pgcit: &Pgcit,
    pgcn: u16,
    pgn: u16,
) -> Option<(&Pgc, u16, usize, usize)> {
    let pgc = find_pgc_by_id(pgcit, pgcn)?;
    if pgn < 1 || pgn as usize > pgc.program_map.len() {
        return None;
    }
    let start_cell = pgc.program_map[pgn as usize - 1] as usize;
    if start_cell == 0 {
        return None;
    }
    let start = start_cell - 1;
    let end = pgc.nr_cells as usize - 1;
    Some((pgc, pgcn, start, end))
}

fn find_pgc_by_id(pgcit: &Pgcit, pgcn: u16) -> Option<&Pgc> {
    if pgcn == 0 {
        return None;
    }
    pgcit.pgcs.get(pgcn as usize - 1).map(|(_, p)| p)
}

pub(super) fn pgc_has_vob(pgc: &Pgc, start: usize, end: usize, hint: u16) -> bool {
    (start..=end).any(|c| {
        c < pgc.cell_position.len()
            && u16::from_be_bytes([pgc.cell_position[c][0], pgc.cell_position[c][1]]) == hint
    })
}

pub(super) fn cell_duration_sec(pgc: &Pgc, cell: usize) -> f64 {
    dvdtime_to_sec(&pgc.cell_playback[cell][4..8])
}

pub(super) fn cell_first_sector(pgc: &Pgc, cell: usize) -> u32 {
    let b = &pgc.cell_playback[cell];
    u32::from_be_bytes([b[8], b[9], b[10], b[11]])
}

pub(super) fn cell_last_sector(pgc: &Pgc, cell: usize) -> u32 {
    let b = &pgc.cell_playback[cell];
    u32::from_be_bytes([b[20], b[21], b[22], b[23]])
}

pub(super) fn title_playback_sec(pgc: &Pgc, start: usize, end: usize) -> f64 {
    let mut total = 0.0_f64;
    for c in start..=end.min(pgc.cell_playback.len().saturating_sub(1)) {
        let sec = dvdtime_to_sec(&pgc.cell_playback[c][4..8]);
        if sec.is_finite() && sec > 0.0 {
            total += sec;
        }
    }
    total
}

pub(super) fn fill_ptt_marks(
    ptt: &[(u16, u16)],
    pgc: &Pgc,
    pgc_id: u16,
    start_cell: usize,
    end_cell: usize,
    marks: &mut Vec<f64>,
) {
    if ptt.len() < 2 {
        return;
    }
    let mut cell = start_cell;
    let mut t = 0.0_f64;
    for (sj, &(sj_pgc, sj_pgn)) in ptt.iter().enumerate() {
        if sj_pgc != pgc_id || sj_pgn == 0 {
            break;
        }
        if sj_pgn as usize > pgc.program_map.len() {
            break;
        }
        let chapter_cell = pgc.program_map[sj_pgn as usize - 1] as usize;
        let target = chapter_cell.saturating_sub(1);
        while cell < target && cell <= end_cell {
            t += dvdtime_to_sec(&pgc.cell_playback[cell][4..8]);
            cell += 1;
        }
        if sj > 0 && marks.len() < MAX_MARKS {
            marks.push(t);
        }
    }
}
