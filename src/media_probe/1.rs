use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;
use libmpv2::Mpv;

use crate::db;
use crate::paths;

/// Near-end window (seconds); matches [percent_from_resume] and `app` sibling/continue rules.
const NEAR_END: f64 = 3.0;

/// Data for one recent-movie card.
pub struct CardData {
    pub path: PathBuf,
    /// 0.0..=100.0, or 0 if unknown.
    pub percent: f64,
    /// Image bytes (JPEG/PNG, etc.), or [None] to show the generic video icon.
    pub thumb: Option<Vec<u8>>,
    /// File missing; card is greyed and click removes the entry.
    pub missing: bool,
}

fn read_start_from_wl(p: &Path) -> Option<f64> {
    let f = std::fs::File::open(p).ok()?;
    for line in BufReader::new(f).lines().map_while(|l| l.ok()) {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("start=") {
            return v.trim().parse().ok();
        }
    }
    None
}

/// Find mpv’s watch_later file for this path. libmpv 0.35+ with
/// `write-filename-in-watch-later-config` stores the path in a line `# /full/path` (not `filename=…`).
/// Older or other builds may use `filename=…` — we accept both.
/// Remove per-file `watch_later` sidecar and DB `time_pos` so the next `loadfile` starts at 0.
pub fn clear_resume_for_path(media: &Path) {
    if let Some(wl) = watch_later_config_for(media) {
        let _ = std::fs::remove_file(wl);
    }
    db::clear_resume_position(media);
}

/// Clear watch-later/DB resume, then drop [path] from continue **history** (dismiss, trash, EOF with no next, etc.).
pub fn remove_continue_entry(path: &Path) {
    clear_resume_for_path(path);
    crate::history::remove(path);
}

/// In-memory token so **Undo** after “remove from list” can put back resume + `media` cache.
#[derive(Debug, Clone)]
pub struct ListRemoveUndo {
    pub path: PathBuf,
    /// Exact watch_later file path and bytes, if it existed.
    pub watch_later: Option<(PathBuf, Vec<u8>)>,
    /// Full SQLite `media` row for this path, if any.
    pub media: Option<db::MediaRowSnapshot>,
}

/// Call **before** [remove_continue_entry] for a manual dismiss.
pub fn capture_list_remove_undo(path: &Path) -> ListRemoveUndo {
    let path = path.to_path_buf();
    let watch_later =
        watch_later_config_for(&path).and_then(|p| std::fs::read(&p).ok().map(|b| (p, b)));
    let media = db::snapshot_media_row(&path);
    ListRemoveUndo {
        path,
        watch_later,
        media,
    }
}

/// Restore sidecar + DB; caller re-adds history via [crate::history::record].
pub fn restore_list_remove_undo(s: &ListRemoveUndo) {
    if let Some((ref p, ref bytes)) = s.watch_later {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, bytes);
    }
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

/// When switching the loaded file: treat as "done" for continue + resume if [is_natural_end] **or** the
/// user is in the last **~15%** of a long enough file (so **Next** at end credits, where `time-pos` is
/// still far from the muxed `duration`, still drops the title from the continue list).
pub fn is_done_enough_to_drop_continue(mpv: &Mpv) -> bool {
    if is_natural_end(mpv) {
        return true;
    }
    let (Ok(pos), Ok(dur)) = (
        mpv.get_property::<f64>("time-pos"),
        mpv.get_property::<f64>("duration"),
    ) else {
        return false;
    };
    if !pos.is_finite() || !dur.is_finite() || dur < 30.0 {
        return false;
    }
    dur > 60.0 && pos / dur >= 0.85
}

/// Match `s` to watch_later file contents (same as [watch_later_config_for] for a path string when the file is gone).
fn watch_later_file_matching_path_stored(s: &str) -> Option<PathBuf> {
    let dir = paths::watch_later()?;
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        if let Ok(txt) = std::fs::read_to_string(&p) {
            for line in txt.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("filename=") {
                    if rest == s {
                        return Some(p);
                    }
                }
                if let Some(rest) = t.strip_prefix("#") {
                    if rest.trim() == s {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn watch_later_config_for(media: &Path) -> Option<PathBuf> {
    let s: String = if let Ok(can) = std::fs::canonicalize(media) {
        can.to_str()?.to_string()
    } else {
        media.to_str()?.to_string()
    };
    watch_later_file_matching_path_stored(s.as_str())
}

fn resume_start_seconds(path: &Path) -> Option<f64> {
    let wl = watch_later_config_for(path)?;
    read_start_from_wl(&wl)
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
const GRID_THUMB_W: u32 = 480;
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

/// DB hit for a **canonical** path (avoids a second [canonicalize] in [ensure_thumbnail]).
fn thumb_time_for_path(path: &Path, key: &str) -> f64 {
    let target = db::load_time_pos_map()
        .get(key)
        .copied()
        .or_else(|| resume_start_seconds(path))
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
    let t = thumb_time_for_path(can, s);
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
