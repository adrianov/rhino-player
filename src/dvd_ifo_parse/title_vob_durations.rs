// Per-`.vob` segment lengths from `VTS_xx_0.IFO` PGC cell playback times.

use std::path::{Path, PathBuf};

#[path = "title_pgc.rs"]
mod title_pgc;
#[path = "title_vob_sector_durs.rs"]
mod title_vob_sector_durs;

use title_pgc::title_cells;

/// Whole-title PGC playback length for one VTS title track (`TTN`, 1-based).
#[must_use]
pub fn title_ttn_playback_sec(ifo_path: &Path, ttn: usize) -> Option<f64> {
    title_pgc::title_ttn_playback_sec(ifo_path, ttn)
}

/// Minimum IFO segment length to treat a `.vob` as the title entry (skips 1 s menu stubs).
pub const MIN_SUBSTANTIAL_SEC: f64 = 5.0;

/// Whole-title seconds from IFO for one title set (`VTS_xx_*` only).
#[must_use]
pub fn title_set_playback_sec(chapter_vob: &Path) -> Option<f64> {
    let cells = title_cells(chapter_vob)?;
    let total: f64 = cells.iter().map(|c| c.dur_sec).sum();
    (total > 0.0).then_some(total)
}

/// Per on-disk chapter `.vob` durations for the disc feature queue (`timeline_chapter_paths`).
pub fn title_vob_durations(chapter_vob: &Path) -> Option<Vec<f64>> {
    let paths = crate::dvd_entity::timeline_chapter_paths(chapter_vob)?;
    map_disc_paths_by_sector(&paths)
}

fn map_disc_paths_by_sector(paths: &[PathBuf]) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(paths.len());
    let mut i = 0;
    while i < paths.len() {
        let tid = crate::dvd_entity::vob_title_id(&paths[i])?;
        let mut j = i + 1;
        while j < paths.len() && crate::dvd_entity::vob_title_id(&paths[j]) == Some(tid) {
            j += 1;
        }
        let block = &paths[i..j];
        let cells = title_cells(&block[0])?;
        let block_durs = title_vob_sector_durs::map_cells_by_sector(&cells, block)?;
        if block_durs.len() != block.len() {
            return None;
        }
        out.extend(block_durs);
        i = j;
    }
    Some(out)
}

/// Whole-title seconds from IFO cell playback times (sum of all cells in the main PGC).
#[must_use]
pub fn title_playback_sec(chapter_vob: &Path) -> Option<f64> {
    let total: f64 = title_vob_durations(chapter_vob)?.into_iter().sum();
    (total > 0.0).then_some(total)
}

/// First chapter file with meaningful IFO content (skips short stubs on `VTS_xx_1.VOB`).
#[must_use]
pub fn first_substantial_vob(chapter_vob: &Path) -> Option<PathBuf> {
    let paths = crate::dvd_entity::title_chapter_paths(chapter_vob)?;
    let durs = title_vob_durations(chapter_vob)?;
    if let Some(first) = paths.first() {
        if durs.first().copied().unwrap_or(0.0) <= MIN_SUBSTANTIAL_SEC
            && crate::dvd_entity::chapter_vob_substantial_on_disk(first)
        {
            return Some(first.clone());
        }
    }
    if let Some((p, _)) = paths
        .iter()
        .zip(durs.iter())
        .find(|(_, d)| **d > MIN_SUBSTANTIAL_SEC)
    {
        return Some(p.clone());
    }
    paths.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn fritt_ifo_vob_durations_when_mounted() {
        let vob = Path::new(
            "/Volumes/SanDisk/Torrents/Fritt.vilt.2006.DVD9/VIDEO_TS/VTS_01_1.VOB",
        );
        if !vob.is_file() {
            return;
        }
        let paths = crate::dvd_entity::timeline_chapter_paths(vob).expect("paths");
        let durs = title_vob_durations(vob).expect("ifo durs");
        assert_eq!(durs.len(), paths.len());
        let total: f64 = durs.iter().sum();
        assert!(
            (total - 5842.0).abs() < 5.0,
            "IFO title length should be ~97 min, got {total:.1}s"
        );
        assert!(
            durs[0] > 1050.0 && durs[0] < 1080.0,
            "VTS_01_1 sector split should be ~1062s, got {:.1}s",
            durs[0]
        );
        let tl = crate::dvd_vob_timeline::DvdVobTimeline::from_title_vobs_with(
            vob,
            &std::collections::HashMap::new(),
            0.0,
            crate::dvd_entity::TimelineBuildOpts::CACHE_ONLY,
        )
        .expect("timeline");
        assert!(
            (tl.total_sec - total).abs() < 1.0,
            "bar total should match IFO sector sum, got {:.1}s vs {total:.1}s",
            tl.total_sec
        );
        let first = first_substantial_vob(vob).expect("first");
        assert_eq!(
            first.file_name().and_then(|n| n.to_str()),
            Some("VTS_01_1.VOB")
        );
    }
}
