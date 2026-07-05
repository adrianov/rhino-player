use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use libmpv2::Mpv;

use crate::db;

/// Near-end window (seconds); matches [percent_from_resume] and `app` sibling/continue rules.
pub const NEAR_END_SEC: f64 = 3.0;
const NEAR_END: f64 = NEAR_END_SEC;

/// Resume + duration (seconds) for one continue card — filled once with the grid, reused by transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContinueSnap {
    pub resume_sec: f64,
    pub duration_sec: f64,
}

/// Canonical path string → snap; rebuilt whenever the continue row is filled ([continue_grid_cache_refresh]).
pub type ContinueGridCache = Rc<RefCell<HashMap<String, ContinueSnap>>>;

/// Rebuild the cache from [CardData] (two SQLite reads happen only in [crate::media_probe::card_data_list]).
pub fn continue_grid_cache_refresh(cache: &ContinueGridCache, cards: &[CardData]) {
    let mut g = cache.borrow_mut();
    g.clear();
    for c in cards {
        if c.missing {
            continue;
        }
        let Some(k) = crate::db::history_key(&c.path) else {
            continue;
        };
        g.insert(
            k,
            ContinueSnap {
                resume_sec: c.resume_sec,
                duration_sec: c.duration_sec,
            },
        );
    }
}

pub fn continue_grid_cache_lookup(cache: &ContinueGridCache, path: &Path) -> Option<ContinueSnap> {
    let key = crate::db::history_key(path)?;
    cache.borrow().get(&key).copied()
}

/// Register the live continue-grid cache so seek / transport persist can refresh browse snaps.
pub fn continue_grid_cache_attach(cache: ContinueGridCache) {
    continue_grid_cache_hook::attach(cache);
}

/// Keep browse-overlay snap in sync after a live seek / transport persist (avoids stale rewind).
pub fn continue_grid_cache_note_playback(entity: &Path, resume_sec: f64, duration_sec: f64) {
    continue_grid_cache_hook::note(entity, resume_sec, duration_sec);
}

/// Data for one recent-movie card.
pub struct CardData {
    pub path: PathBuf,
    /// 0.0..=100.0, or 0 if unknown.
    pub percent: f64,
    /// WebP thumbnail bytes, or [None] to show the generic video icon.
    pub thumb: Option<Vec<u8>>,
    /// File missing; card is greyed and click removes the entry.
    pub missing: bool,
    pub resume_sec: f64,
    pub duration_sec: f64,
}

/// Drop SQLite resume position so the next `loadfile` starts at 0.
pub fn clear_resume_for_path(media: &Path) {
    crate::playback_entity::clear_entity_resume(media);
}

/// Clear DB resume, then drop [path] from continue **history** (dismiss, trash, EOF with no next, etc.).
pub fn remove_continue_entry(path: &Path) {
    let entity = crate::playback_entity::db_path_for(path);
    clear_resume_for_path(&entity);
    crate::history::remove(path);
}

/// In-memory token so **Undo** after "remove from list" can put back the SQLite `media` row.
#[derive(Debug, Clone)]
pub struct ListRemoveUndo {
    pub path: PathBuf,
    /// Full SQLite `media` row for this path, if any.
    pub media: Option<db::MediaRowSnapshot>,
}

/// Call **before** [remove_continue_entry] for a manual dismiss.
pub fn capture_list_remove_undo(path: &Path) -> ListRemoveUndo {
    let path = crate::playback_entity::db_path_for(path);
    let media = db::snapshot_media_row(&path);
    ListRemoveUndo { path, media }
}

/// Restore SQLite row; caller re-adds history via [crate::history::record].
pub fn restore_list_remove_undo(s: &ListRemoveUndo) {
    if let Some(ref m) = s.media {
        db::apply_media_snapshot(m);
    }
}

/// True at EOF or in the last ~3s of a known duration (same rule as the continue / sibling queue).
pub fn is_natural_end(mpv: &Mpv) -> bool {
    if mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
        return true;
    }
    match (
        mpv.get_property::<f64>("time-pos"),
        mpv.get_property::<f64>("duration"),
    ) {
        (Ok(p), Ok(d)) if p.is_finite() && d > 0.0 => d - p <= NEAR_END,
        _ => false,
    }
}

fn percent_from_resume(start: Option<f64>, duration: Option<f64>) -> f64 {
    match (start, duration) {
        (Some(s), Some(d)) if d > 0.0 => {
            if s >= d - NEAR_END && d > 5.0 {
                100.0
            } else {
                (100.0 * s / d).clamp(0.0, 100.0)
            }
        }
        _ => 0.0,
    }
}

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

/// Map entity resume to the chapter file + local seek used for continue-grid thumbs.
fn grid_thumb_target(entity: &Path) -> Option<GridThumbTarget> {
    if !entity.exists() {
        return None;
    }
    let pe = crate::playback_entity::PlaybackEntity::resolve(entity);
    let db_key = pe.db_path();
    let durs = db::load_duration_map();
    let tpos = db::load_time_pos_map();
    let (resume, duration) = crate::playback_entity::card_resume_duration(&db_key, &durs, &tpos);
    let cache_time = grid_thumb_cache_time(resume, duration);
    let open_hint = crate::video_ext::resolve_open_media_path(entity);
    if pe.has_unified_timeline() {
        let probe = crate::dvd_entity::timeline_chapter_probe(&open_hint)
            .unwrap_or_else(|| open_hint.clone());
        let still = pe.still_at_global(&probe, cache_time, &durs, None, None)?;
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
        return Some(GridThumbTarget {
            load,
            seek_sec,
            chapter_dur: still.chapter_dur,
            cache_time,
        });
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
    cached_thumbnail_fresh(path).or_else(|| {
        db::stored_thumb_webp(&entity).and_then(|b| {
            if crate::thumb_texture::thumb_webp_is_flat_fill(&b) {
                None
            } else {
                Some(b)
            }
        })
    })
}
