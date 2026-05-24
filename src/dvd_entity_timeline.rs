fn chapter_dur_from_map(path: &Path, dur_by_path: &HashMap<String, f64>) -> f64 {
    if let Some(s) = path.to_str() {
        if let Some(d) = dur_by_path.get(s).copied() {
            if d.is_finite() && d > 0.0 {
                return d;
            }
        }
    }
    if let Ok(c) = std::fs::canonicalize(path) {
        if let Some(cs) = c.to_str() {
            if let Some(d) = dur_by_path.get(cs).copied() {
                if d.is_finite() && d > 0.0 {
                    return d;
                }
            }
        }
    }
    0.0
}

/// Unified timeline from on-disk title `.vob` queue and per-file durations only.
pub(crate) fn build_title_timeline(
    chapter: &Path,
    dur_by_path: &HashMap<String, f64>,
    live_local_dur: f64,
) -> Option<DvdVobTimeline> {
    DvdVobTimeline::from_title_vobs(
        chapter,
        dur_by_path,
        Some(chapter),
        live_local_dur,
    )
}

/// Global title time and total duration for persistence (seconds).
pub(crate) fn playback_snapshot(
    chapter: &Path,
    local_pos: f64,
    local_dur: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    let tl = build_title_timeline(chapter, dur_by_path, local_dur)?;
    let global = tl.global_pos(chapter, local_pos);
    let total = tl.total_sec.max(local_dur);
    Some((total, global))
}

/// Continue-grid / `vo=image` target: `.vob` file, local offset, and segment length cap.
pub(crate) struct DvdStillTarget {
    pub load: PathBuf,
    pub local_sec: f64,
    pub chapter_dur: f64,
}

fn still_target_at_chapter(
    tl: &DvdVobTimeline,
    idx: usize,
    local: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<DvdStillTarget> {
    let load = tl.path_at(idx)?.to_path_buf();
    let mut chapter_dur = tl.chapter_dur_at(idx);
    let mapped = crate::dvd_vob_timeline::dur_from_map(dur_by_path, &load);
    if mapped > 0.0 {
        chapter_dur = if chapter_dur > 0.0 {
            chapter_dur.min(mapped)
        } else {
            mapped
        };
    } else if chapter_dur <= 0.0 {
        chapter_dur = chapter_dur_from_map(&load, dur_by_path);
    }
    let cap = if chapter_dur > 0.0 {
        chapter_dur
    } else {
        local + 1.0
    };
    let local_sec = crate::seek_bar_preview::cap_preview_seek_time(local, cap);
    Some(DvdStillTarget {
        load,
        local_sec,
        chapter_dur,
    })
}

/// Map whole-title seconds to a `.vob` load + local seek.
pub(crate) fn still_target_from_global(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<DvdStillTarget> {
    let live = chapter_dur_from_map(chapter, dur_by_path);
    let tl = build_title_timeline(chapter, dur_by_path, live)?;
    let g = global_sec.clamp(0.0, tl.total_sec);
    let (idx, local) = tl.resolve_global(g);
    still_target_at_chapter(&tl, idx, local, dur_by_path)
}

/// Map stored global resume to `(vob path, local offset)` for `loadfile`.
pub(crate) fn resume_chapter_and_local(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(PathBuf, f64)> {
    still_target_from_global(chapter, global_sec, dur_by_path)
        .map(|t| (t.load, t.local_sec))
}
