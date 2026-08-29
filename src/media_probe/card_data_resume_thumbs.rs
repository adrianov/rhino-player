use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
// Shared with later `include!` siblings in `media_probe` (thumb capture waits).
use std::time::{Duration, Instant};

use libmpv2::Mpv;

use crate::db;

/// Near-end window (seconds); matches [percent_from_resume] and `app` sibling/continue rules.
pub const NEAR_END_SEC: f64 = 3.0;
const NEAR_END: f64 = NEAR_END_SEC;

/// Progress fraction at which a media switch clears the continue card (credits / recaps).
/// See [is_continue_done].
pub const CONTINUE_DONE_FRAC: f64 = 0.85;
/// Titles shorter than this never use [CONTINUE_DONE_FRAC] (seconds).
pub const CONTINUE_DONE_MIN_SEC: f64 = 60.0;

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

/// Strip cache first; on miss, one-row SQLite resume/duration (search hits not in the continue five).
pub fn continue_snap_for_browse(cache: &ContinueGridCache, path: &Path) -> Option<ContinueSnap> {
    if let Some(s) = continue_grid_cache_lookup(cache, path) {
        return Some(s);
    }
    let entity = crate::playback_entity::db_path_for(path);
    let resume_sec = db::resume_pos(&entity).unwrap_or(0.0);
    let duration_sec = db::media_duration_sec(&entity).unwrap_or(0.0);
    if resume_sec <= 0.0 && duration_sec <= 0.0 {
        return None;
    }
    let snap = ContinueSnap {
        resume_sec,
        duration_sec,
    };
    if let Some(k) = crate::db::history_key(&entity) {
        cache.borrow_mut().insert(k, snap);
    }
    Some(snap)
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

/// True when a media switch should clear this title from Continue: natural end, or past
/// [CONTINUE_DONE_FRAC] on a long enough single file (skipping credits). Incomplete downloads
/// and multi-part DVD titles skip the fraction gate — demux length is not the whole title.
pub fn is_continue_done(mpv: &Mpv) -> bool {
    if is_natural_end(mpv) {
        return true;
    }
    let Some(path) = local_file_from_mpv(mpv) else {
        return false;
    };
    if crate::human_media_title::is_incomplete_download_path(&path)
        || crate::playback_entity::PlaybackEntity::resolve(&path).has_unified_timeline()
    {
        return false;
    }
    let (Ok(pos), Ok(dur)) = (
        mpv.get_property::<f64>("time-pos"),
        mpv.get_property::<f64>("duration"),
    ) else {
        return false;
    };
    past_done_mark(pos, dur)
}

/// Pure fraction gate for [is_continue_done] (unit-tested).
pub(crate) fn past_done_mark(pos: f64, dur: f64) -> bool {
    pos.is_finite()
        && dur.is_finite()
        && dur > CONTINUE_DONE_MIN_SEC
        && pos / dur >= CONTINUE_DONE_FRAC
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

#[cfg(test)]
mod continue_done_tests {
    use super::*;

    #[test]
    fn credits_count_as_done() {
        assert!(past_done_mark(55.0 * 60.0 * 0.92, 55.0 * 60.0));
        assert!(past_done_mark(85.0, 100.0));
    }

    #[test]
    fn mid_title_and_short_clips_stay() {
        assert!(!past_done_mark(40.0 * 60.0, 55.0 * 60.0));
        assert!(!past_done_mark(50.0, 55.0));
        assert!(!past_done_mark(f64::NAN, 120.0));
    }
}
