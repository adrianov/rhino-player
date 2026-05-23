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

fn entity_total_from_map(entity: &Path, dur_by_path: &HashMap<String, f64>) -> Option<f64> {
    let keys = [entity.to_path_buf()].into_iter().chain(
        std::fs::canonicalize(entity)
            .ok()
            .into_iter()
            .filter(|c| !crate::video_ext::paths_same_file(c, entity)),
    );
    for k in keys {
        if let Some(s) = k.to_str() {
            if let Some(d) = dur_by_path.get(s).copied() {
                if d.is_finite() && d > 0.0 {
                    return Some(d);
                }
            }
        }
    }
    None
}

/// Shared DVD title timeline for resume mapping, persistence, and continue-grid thumbs.
pub(crate) fn build_title_timeline(
    chapter: &Path,
    dur_by_path: &HashMap<String, f64>,
    live_local_dur: f64,
) -> Option<DvdVobTimeline> {
    if !crate::video_ext::is_dvd_vob_path(chapter) {
        return None;
    }
    let mut tl = DvdVobTimeline::from_chapter_ifo(chapter)
        .or_else(|| {
            DvdVobTimeline::from_chapter(chapter, dur_by_path, chapter, live_local_dur)
        })
        .or_else(|| DvdVobTimeline::from_chapter_db_only(chapter, dur_by_path))?;
    if let Some(on_disk) = title_chapter_paths(chapter) {
        tl.expand_on_disk_chapters(&on_disk);
    }
    tl.apply_map_chapter_durs(dur_by_path);
    let entity = crate::playback_entity::db_path_for(chapter);
    let title_total = entity_total_from_map(&entity, dur_by_path);
    let bootstrap_live = if live_local_dur > 0.0 {
        live_local_dur
    } else {
        chapter_dur_from_map(chapter, dur_by_path)
    };
    if tl.vobs.len() > 1 {
        tl.ensure_chapter_dur_coverage(title_total.unwrap_or(0.0), chapter, bootstrap_live);
    }
    (tl.total_sec > 0.0).then_some(tl)
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

/// Continue-grid / `vo=image` target: chapter file, local offset, and chapter length cap.
pub(crate) struct DvdStillTarget {
    pub load: PathBuf,
    pub local_sec: f64,
    pub chapter_dur: f64,
}

/// Map whole-title seconds to a chapter `.vob` load + local seek (same timeline as preview/resume).
pub(crate) fn still_target_from_global(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<DvdStillTarget> {
    let live = chapter_dur_from_map(chapter, dur_by_path);
    let tl = build_title_timeline(chapter, dur_by_path, live)?;
    let g = global_sec.clamp(0.0, tl.total_sec);
    let (idx, local) = tl.resolve_global(g);
    let load = tl.path_at(idx)?.to_path_buf();
    let mut chapter_dur = tl.chapter_dur_at(idx);
    let mapped = crate::dvd_vob_timeline::dur_from_map(dur_by_path, &load);
    if mapped > 0.0 {
        chapter_dur = chapter_dur.max(mapped);
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

/// Map stored global resume to `(chapter path, local offset)` for `loadfile`.
pub(crate) fn resume_chapter_and_local(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(PathBuf, f64)> {
    still_target_from_global(chapter, global_sec, dur_by_path)
        .map(|t| (t.load, t.local_sec))
}
