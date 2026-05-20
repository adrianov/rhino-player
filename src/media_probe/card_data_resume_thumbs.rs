use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;
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
        let Some(k) = c.path.to_str() else {
            continue;
        };
        g.insert(
            k.to_string(),
            ContinueSnap {
                resume_sec: c.resume_sec,
                duration_sec: c.duration_sec,
            },
        );
    }
}

pub fn continue_grid_cache_lookup(cache: &ContinueGridCache, path: &Path) -> Option<ContinueSnap> {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let k = abs.to_str()?;
    cache.borrow().get(k).copied()
}

/// Data for one recent-movie card.
pub struct CardData {
    pub path: PathBuf,
    /// 0.0..=100.0, or 0 if unknown.
    pub percent: f64,
    /// Image bytes (JPEG/PNG, etc.), or [None] to show the generic video icon.
    pub thumb: Option<Vec<u8>>,
    /// File missing; card is greyed and click removes the entry.
    pub missing: bool,
    pub resume_sec: f64,
    pub duration_sec: f64,
}

/// Drop SQLite resume position so the next `loadfile` starts at 0.
pub fn clear_resume_for_path(media: &Path) {
    db::clear_resume_position(media);
}

/// Clear DB resume, then drop [path] from continue **history** (dismiss, trash, EOF with no next, etc.).
pub fn remove_continue_entry(path: &Path) {
    clear_resume_for_path(path);
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
    let path = path.to_path_buf();
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

/// Continue-grid backfill: cap generated width near card size and let GTK cover-scale if needed.
const GRID_THUMB_W: u32 = 960;
const GRID_FALLBACK_SEC: f64 = 2.0;

/// Hash for cache filename (FNV-1a on UTF-8 path bytes).
fn path_tag(path: &str) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut h = OFFSET;
    for b in path.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Wanted thumb time for a **canonical** path (DB key is the canonical path string).
fn thumb_time_for_path(key: &str) -> f64 {
    let target = db::load_time_pos_map()
        .get(key)
        .copied()
        .unwrap_or(GRID_FALLBACK_SEC);
    let dur = db::load_duration_map().get(key).copied().unwrap_or(0.0);
    if dur.is_finite() && dur > 1.0 {
        target.clamp(0.0, (dur - 0.5).max(0.0))
    } else {
        target.max(0.0)
    }
}

fn db_thumb_for_canon_path(can: &Path) -> Option<Vec<u8>> {
    let s = can.to_str()?;
    let mtime = db::file_mtime_sec(can)?;
    let t = thumb_time_for_path(s);
    db::take_thumb_if_fresh(s, mtime, t)
}

/// Current thumbnail for this path in [crate::db] when [db::file_mtime_sec] matches; **no libmpv** (use on the UI thread).
pub fn cached_thumbnail_for_path(path: &Path) -> Option<Vec<u8>> {
    if !path.exists() {
        return None;
    }
    let can = std::fs::canonicalize(path).ok()?;
    db_thumb_for_canon_path(&can)
}

/// Display fallback: show the last valid raster for this file while background backfill refreshes
/// a stale `thumb_time_pos_sec`.
fn cached_thumbnail_for_display(path: &Path) -> Option<Vec<u8>> {
    if !path.exists() {
        return None;
    }
    let can = std::fs::canonicalize(path).ok()?;
    let s = can.to_str()?;
    let mtime = db::file_mtime_sec(&can)?;
    db::take_thumb_if_current(s, mtime)
}
