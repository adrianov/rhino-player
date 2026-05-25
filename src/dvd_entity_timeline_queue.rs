// Disc-wide unified timeline queue: consecutive substantial title sets on one rip.

/// Minimum IFO title-set length to join the unified disc timeline (excludes menus / stubs).
pub(crate) const MIN_TIMELINE_TITLE_SET_SEC: f64 = 300.0;

/// Feature chapter `.vob` queue for the unified timeline on this disc.
pub(crate) fn timeline_chapter_paths(path: &Path) -> Option<Vec<PathBuf>> {
    let tid = vob_title_id(path)?;
    let vts_dir = video_ts_for_vob(path)?;
    let run = contiguous_substantial_title_run(&vts_dir, tid)?;
    let mut out = Vec::new();
    for id in run {
        out.extend(chapter_vobs_for_title(&vts_dir, id));
    }
    (!out.is_empty()).then_some(out)
}

fn title_ids_on_disc(vts_dir: &Path) -> Vec<u32> {
    let Ok(read) = std::fs::read_dir(vts_dir) else {
        return Vec::new();
    };
    let mut ids: Vec<u32> = read
        .flatten()
        .filter_map(|e| vob_title_id(&e.path()))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn is_substantial_title_set(vts_dir: &Path, title_id: u32) -> bool {
    let vobs = chapter_vobs_for_title(vts_dir, title_id);
    let Some(first) = vobs.first() else {
        return false;
    };
    crate::dvd_ifo_parse::title_set_playback_sec(first)
        .is_some_and(|s| s >= MIN_TIMELINE_TITLE_SET_SEC)
}

fn contiguous_substantial_title_run(vts_dir: &Path, open_tid: u32) -> Option<Vec<u32>> {
    let substantial: Vec<u32> = title_ids_on_disc(vts_dir)
        .into_iter()
        .filter(|id| is_substantial_title_set(vts_dir, *id))
        .collect();
    if substantial.is_empty() {
        return contiguous_title_id_run(vts_dir, open_tid);
    }
    let Some(pos) = substantial.iter().position(|&id| id == open_tid) else {
        return Some(vec![open_tid]);
    };
    let mut start = pos;
    let mut end = pos;
    while start > 0 && substantial[start] == substantial[start - 1] + 1 {
        start -= 1;
    }
    while end + 1 < substantial.len() && substantial[end + 1] == substantial[end] + 1 {
        end += 1;
    }
    Some(substantial[start..=end].to_vec())
}

fn contiguous_title_id_run(vts_dir: &Path, open_tid: u32) -> Option<Vec<u32>> {
    let ids = title_ids_on_disc(vts_dir);
    let pos = ids.iter().position(|&id| id == open_tid)?;
    let mut end = pos;
    while end + 1 < ids.len() && ids[end + 1] == ids[end] + 1 {
        end += 1;
    }
    Some(ids[pos..=end].to_vec())
}
