// Title-wide global seconds → chapter `.vob` + IFO-local seek (preview, continue grid, resume).

/// Optional live-open cap (preview hover on the chapter mpv is decoding).
pub(crate) struct StillOpenCap {
    pub chapter: std::path::PathBuf,
    pub mpv_dur: f64,
}

fn chapter_dur_for_still(
    tl: &DvdVobTimeline,
    bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    global: f64,
    idx: usize,
    local: f64,
    load: &Path,
    dur_by_path: &HashMap<String, f64>,
) -> f64 {
    if let Some(b) = bar {
        return crate::dvd_vob_timeline::preview_chapter_dur(b, global, idx, local, load, dur_by_path);
    }
    let mut dur = tl.chapter_dur_at(idx);
    let mapped = crate::dvd_vob_timeline::dur_from_map(dur_by_path, load);
    if mapped > 0.0 {
        dur = if dur > 0.0 { dur.min(mapped) } else { mapped };
    } else if dur <= 0.0 {
        dur = chapter_dur_from_map(load, dur_by_path);
    }
    dur.max(0.0)
}

/// Single mapping for seek preview, continue-grid thumbs, and resume load targets.
pub(crate) fn still_at_global(
    probe: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
    bar: Option<&crate::dvd_vob_timeline::DvdBarState>,
    open_cap: Option<&StillOpenCap>,
) -> Option<DvdStillTarget> {
    let live = chapter_dur_from_map(probe, dur_by_path);
    let built = build_title_timeline_with(probe, dur_by_path, live, TimelineBuildOpts::CACHE_ONLY);
    let tl = bar.map(|b| &b.tl).or(built.as_ref())?;
    let total = tl.total_sec;
    if !(total > 0.0) {
        return None;
    }
    let g = global_sec.clamp(0.0, total);
    let (idx, mut local) = tl.resolve_global(g);
    let load = tl.path_at(idx)?;
    let mut chapter_dur = chapter_dur_for_still(tl, bar, g, idx, local, load, dur_by_path);
    if let Some(open) = open_cap {
        if crate::video_ext::paths_same_file(load, &open.chapter) {
            let cap = crate::dvd_vob_timeline::clamp_vob_duration(open.mpv_dur);
            if cap > 0.0 {
                chapter_dur = chapter_dur.min(cap);
                local = local.min((cap - 0.05).max(0.0));
            }
        }
    }
    still_target_at_chapter(tl, idx, local, dur_by_path, Some(chapter_dur))
}
