fn chapter_dur_from_map(path: &Path, dur_by_path: &HashMap<String, f64>) -> f64 {
    if let Some(s) = path.to_str() {
        if let Some(d) = dur_by_path.get(s).copied() {
            let d = crate::dvd_vob_timeline::clamp_vob_duration(d);
            if d > 0.0 {
                return d;
            }
        }
    }
    if let Ok(c) = std::fs::canonicalize(path) {
        if let Some(cs) = c.to_str() {
            if let Some(d) = dur_by_path.get(cs).copied() {
                let d = crate::dvd_vob_timeline::clamp_vob_duration(d);
                if d > 0.0 {
                    return d;
                }
            }
        }
    }
    0.0
}

include!("dvd_entity_timeline_build.rs");

/// Global title time and total duration for persistence (seconds).
pub(crate) fn playback_snapshot(
    chapter: &Path,
    local_pos: f64,
    local_dur: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    let live_dur = crate::dvd_vob_timeline::clamp_vob_duration(local_dur);
    let tl = build_title_timeline_with(chapter, dur_by_path, live_dur, TimelineBuildOpts::CACHE_ONLY)?;
    let local = crate::dvd_vob_timeline::timeline_local_from_mpv(&tl, chapter, local_pos, local_dur);
    let global = tl.global_pos(chapter, local);
    let total = tl.total_sec.max(live_dur);
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
    chapter_dur_override: Option<f64>,
) -> Option<DvdStillTarget> {
    let load = tl.path_at(idx)?.to_path_buf();
    let mut chapter_dur = chapter_dur_override.unwrap_or_else(|| tl.chapter_dur_at(idx));
    if chapter_dur_override.is_none() {
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

include!("dvd_entity_still.rs");

/// Resume load: probe only the chapter prefix needed to map stored global time.
pub(crate) fn resume_still_target_from_global(
    chapter: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<DvdStillTarget> {
    let t0 = std::time::Instant::now();
    let live = chapter_dur_from_map(chapter, dur_by_path);
    let Some(mut tl) = build_title_timeline_with(chapter, dur_by_path, live, TimelineBuildOpts::CACHE_ONLY)
    else {
        crate::dvd_vob_log::resume_open_log(format!(
            "resume_still no timeline chapter={}",
            chapter.display()
        ));
        return None;
    };
    let probed = if tl.can_resolve_global(global_sec) {
        0
    } else {
        tl.probe_prefix_for_global(global_sec)
    };
    tl.scrub_implausible_durs();
    tl.infer_missing_from_siblings();
    let ms = t0.elapsed().as_millis();
    if probed > 0 || ms > 50 {
        eprintln!(
            "[rhino] load: dvd_resume ms={ms} probed={probed} global={global_sec:.1} file={}",
            chapter.file_name().and_then(|n| n.to_str()).unwrap_or("?")
        );
    }
    let g = global_sec.clamp(0.0, tl.total_sec);
    let (idx, local) = tl.resolve_global(g);
    still_target_at_chapter(&tl, idx, local, dur_by_path, None)
}

/// Map stored global resume to `(vob path, local offset)` for `loadfile`.
pub(crate) fn resume_chapter_and_local(
    opened: &Path,
    global_sec: f64,
    dur_by_path: &HashMap<String, f64>,
) -> Option<(PathBuf, f64)> {
    let chapter = timeline_chapter_probe(opened)?;
    resume_still_target_from_global(&chapter, global_sec, dur_by_path)
        .map(|t| (t.load, t.local_sec))
}
