// Continue-grid thumbnail cache keys and fresh/display lookups (no libmpv).

/// Continue-grid backfill width (~[crate::recent_view::card_dims::CARD_MAX_W]); cover-scale in GTK.
const GRID_THUMB_W: u32 = 640;
const GRID_FALLBACK_SEC: f64 = 2.0;

/// Wanted continue time for cache keys (whole-title seconds on DVD).
fn grid_thumb_cache_time(resume: f64, duration: f64) -> f64 {
    let target = if resume > 0.0 {
        resume
    } else {
        GRID_FALLBACK_SEC
    };
    // Global DVD resume can exceed a stale entity `duration_sec` (first-chapter length only).
    if duration.is_finite() && duration > 1.0 && resume <= duration + 0.5 {
        target.clamp(0.0, (duration - 0.5).max(0.0))
    } else {
        target.max(0.0)
    }
}

struct GridThumbTarget {
    load: PathBuf,
    /// Seconds to seek inside [Self::load] for screenshot-raw capture.
    seek_sec: f64,
    /// Chapter length used to cap the seek (preview uses the same rule).
    chapter_dur: f64,
    /// Whole-title seconds stored in `thumb_time_pos_sec` for cache freshness.
    cache_time: f64,
}

/// Continue state for an entity db key: cache key time, duration, and the duration map
/// reused by unified-timeline still mapping.
fn entity_continue_state(db_key: &Path) -> (f64, f64, std::collections::HashMap<String, f64>) {
    let durs = db::load_duration_map();
    let tpos = db::load_time_pos_map();
    let (resume, duration) = crate::playback_entity::card_resume_duration(db_key, &durs, &tpos);
    let cache_time = grid_thumb_cache_time(resume, duration);
    (cache_time, duration, durs)
}

/// Unified-timeline branch of [grid_thumb_target]: map the chapter probe to a still frame.
fn grid_thumb_target_unified(
    pe: &crate::playback_entity::PlaybackEntity,
    open_hint: &Path,
    cache_time: f64,
    durs: &std::collections::HashMap<String, f64>,
) -> Option<GridThumbTarget> {
    let probe = crate::dvd_entity::timeline_chapter_probe(open_hint)
        .unwrap_or_else(|| open_hint.to_path_buf());
    let still = pe.still_at_global(&probe, cache_time, durs, None, None)?;
    let load = std::fs::canonicalize(&still.load).ok()?;
    let seek_sec = if still.local_sec < 0.5 && still.chapter_dur > GRID_FALLBACK_SEC {
        GRID_FALLBACK_SEC
    } else {
        still.local_sec
    };
    crate::dvd_vob_log::dvd_seek_log(format!(
        "grid_thumb global={cache_time:.2} -> {} local={seek_sec:.2} ch_dur={:.2}",
        load.display(),
        still.chapter_dur
    ));
    Some(GridThumbTarget {
        load,
        seek_sec,
        chapter_dur: still.chapter_dur,
        cache_time,
    })
}

/// Map entity resume to the chapter file + local seek used for continue-grid thumbs.
fn grid_thumb_target(entity: &Path) -> Option<GridThumbTarget> {
    if !entity.exists() {
        return None;
    }
    let pe = crate::playback_entity::PlaybackEntity::resolve(entity);
    let (cache_time, duration, durs) = entity_continue_state(&pe.db_path());
    let open_hint = crate::video_ext::resolve_open_media_path(entity);
    if pe.has_unified_timeline() {
        return grid_thumb_target_unified(&pe, &open_hint, cache_time, &durs);
    }
    let load = std::fs::canonicalize(open_hint).ok()?;
    Some(GridThumbTarget {
        load,
        seek_sec: cache_time,
        chapter_dur: duration,
        cache_time,
    })
}

fn db_thumb_for_canon_path(can: &Path) -> Option<Vec<u8>> {
    let s = can.to_str()?;
    let target = grid_thumb_target(can)?;
    db_thumb_for_entity_key(s, &target.load, target.cache_time)
}

/// Thumbnail bytes when cache matches mtime, continue position, and load path; **no libmpv**.
fn cached_thumbnail_fresh(path: &Path) -> Option<Vec<u8>> {
    let entity = crate::playback_entity::db_path_for(path);
    let Some(k) = crate::db::history_key(&entity) else {
        let can = std::fs::canonicalize(path).ok()?;
        return db_thumb_for_canon_path(&can);
    };
    let target = grid_thumb_target(&entity)?;
    db_thumb_for_entity_key(&k, &target.load, target.cache_time)
}

/// Fresh thumb only; used to skip background backfill when regeneration is not needed.
pub fn cached_thumbnail_for_path(path: &Path) -> Option<Vec<u8>> {
    cached_thumbnail_fresh(path)
}

pub(crate) fn db_thumb_for_entity_key(
    db_key: &str,
    load: &Path,
    cache_time: f64,
) -> Option<Vec<u8>> {
    let mtime = db::file_mtime_sec(load)?;
    let load_s = load.to_str();
    let b = db::take_thumb_if_fresh(db_key, mtime, cache_time, load_s)?;
    if crate::thumb_texture::thumb_webp_is_flat_fill(&b) {
        eprintln!("[rhino] grid_thumb reject cached flat fill path={load_s:?}");
        return None;
    }
    Some(b)
}

/// Card art: fresh frame when available, else last stored BLOB (avoids placeholder flash while backfill runs).
pub(crate) fn cached_thumbnail_for_display(path: &Path) -> Option<Vec<u8>> {
    let entity = crate::playback_entity::db_path_for(path);
    cached_thumbnail_fresh(path).or_else(|| db::stored_thumb_webp(&entity))
}
