// Map PGC cell playback times onto physical `.vob` files via IFO sector ranges.

use std::path::PathBuf;

use super::title_pgc::TitleCell;

const DVD_SECTOR_BYTES: u64 = 2048;

/// Per on-disk `.vob` seconds from IFO cell sector overlap (libdvdread model).
pub(super) fn map_cells_by_sector(cells: &[TitleCell], paths: &[PathBuf]) -> Option<Vec<f64>> {
    let bounds = file_sector_bounds(paths)?;
    let mut durs = vec![0.0; paths.len()];
    for cell in cells {
        add_cell_to_files(&mut durs, &bounds, cell);
    }
    Some(durs)
}

fn file_sector_bounds(paths: &[PathBuf]) -> Option<Vec<u64>> {
    let mut bounds = vec![0_u64];
    for path in paths {
        let bytes = path.metadata().ok()?.len();
        let last = bounds.last()?.checked_add(bytes / DVD_SECTOR_BYTES)?;
        bounds.push(last);
    }
    (bounds.len() > 1).then_some(bounds)
}

fn add_cell_to_files(durs: &mut [f64], bounds: &[u64], cell: &TitleCell) {
    let dur = cell.dur_sec;
    if !(dur.is_finite() && dur > 0.0) {
        return;
    }
    let first = u64::from(cell.first_sector);
    let last = u64::from(cell.last_sector);
    if last < first {
        if let Some(i) = file_for_sector(bounds, first) {
            durs[i] += dur;
        }
        return;
    }
    let span = last - first + 1;
    if span == 0 {
        return;
    }
    for (fi, slot) in durs.iter_mut().enumerate() {
        let lo = bounds[fi];
        let hi = bounds[fi + 1];
        if hi <= lo {
            continue;
        }
        let o_start = first.max(lo);
        let o_end = last.min(hi - 1);
        if o_start <= o_end {
            let overlap = o_end - o_start + 1;
            *slot += dur * overlap as f64 / span as f64;
        }
    }
}

fn file_for_sector(bounds: &[u64], sec: u64) -> Option<usize> {
    bounds.windows(2).position(|w| w[0] <= sec && sec < w[1])
}
