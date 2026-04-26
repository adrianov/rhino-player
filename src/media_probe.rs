//! Watch-later resume position, last-known `duration` from libmpv, and **raster** thumbnails (JPEG or PNG) in [crate::db]. The grid/quit paths use a **dedicated in-process [libmpv2::Mpv]** with `vo=image`. See [docs/features/21-recent-videos-launch.md].

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
    let watch_later = watch_later_config_for(&path)
        .and_then(|p| std::fs::read(&p).ok().map(|b| (p, b)));
    let media = db::snapshot_media_row(&path);
    ListRemoveUndo { path, watch_later, media }
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

/// PNG in [crate::db] `media.thumb_png`, rebuilt when the source file’s mtime changes.
/// Calls [run_libmpv_image_frame] on a **cache miss**; keep that work off the UI thread (see [crate::recent_view::schedule_thumb_backfill]).
pub fn ensure_thumbnail(path: &Path) -> Option<Vec<u8>> {
    if !path.exists() {
        return None;
    }
    let can = std::fs::canonicalize(path).ok()?;
    if let Some(t) = db_thumb_for_canon_path(&can) {
        return Some(t);
    }
    let s = can.to_str()?;
    let mtime = db::file_mtime_sec(&can)?;
    let tag = path_tag(s);
    let t = thumb_time_for_path(&can, s);
    let b = run_libmpv_image_frame(&can, tag, t)?;
    db::set_thumb(s, &b, mtime, t);
    Some(b)
}

/// One `vo=image` still (writes into [tmp_dir]), with [loadfile] already applied by the caller, or
/// shared setup through [run_vo_image_one_frame].
fn run_vo_image_after_load(
    m: &mut Mpv,
    tmp: &Path,
    deadline_secs: u64,
) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(deadline_secs);
    let mut end_err = false;
    loop {
        if let Some(f) = pick_vo_out(tmp) {
            if let Some(b) = read_nonempty(&f) {
                return Some(b);
            }
        }
        if Instant::now() > deadline {
            break;
        }
        match m.wait_event(0.1) {
            Some(Err(_)) | None => {}
            Some(Ok(Event::EndFile(r))) => {
                if r == mpv_end_file_reason::Error {
                    end_err = true;
                    break;
                }
            }
            Some(Ok(_)) => {}
        }
    }
    if !end_err {
        for _ in 0..20 {
            if let Some(f) = pick_vo_out(tmp) {
                if let Some(b) = read_nonempty(&f) {
                    return Some(b);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    None
}

/// Thumbnail: resume-position keyframe seek + small scale for continue cards.
fn run_libmpv_image_frame(src: &Path, path_tag: u64, start_sec: f64) -> Option<Vec<u8>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let tmp = std::env::temp_dir().join(format!(
        "rhino-mpv-{}-{}",
        path_tag,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis()
    ));
    let r = run_vo_image_one_frame(
        src,
        &tmp,
        start_sec,
        &format!("scale={GRID_THUMB_W}:-2:force_original_aspect_ratio=decrease:flags=bilinear"),
        12,
    );
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn run_vo_image_one_frame(
    src: &Path,
    tmp: &Path,
    start_sec: f64,
    vf: &str,
    wait_secs: u64,
) -> Option<Vec<u8>> {
    let out_s = tmp.to_str()?;
    let src_s = src.to_str()?;
    std::fs::create_dir_all(tmp).ok()?;
    let start = format!("{:.3}", start_sec);
    let mut m = Mpv::with_initializer(|i| {
        i.set_option("vo", "image")?;
        i.set_option("ao", "null")?;
        let _ = i.set_option("vd-lavc-threads", "0");
        let _ = i.set_option("vd-lavc-fast", true);
        let _ = i.set_option("vd-lavc-skiploopfilter", "all");
        let _ = i.set_option("demuxer-readahead-secs", 0.0f64);
        let _ = i.set_option("demuxer-max-bytes", "128KiB");
        i.set_option("load-scripts", false)?;
        i.set_option("resume-playback", false)?;
        i.set_option("hr-seek", false)?;
        let _ = i.set_option("aid", "no");
        let _ = i.set_option("sid", "no");
        let _ = i.set_option("autoload-files", "no");
        let _ = i.set_option("audio-file-auto", "no");
        let _ = i.set_option("sub-auto", "no");
        i.set_option("vo-image-format", "jpg")?;
        i.set_option("vo-image-outdir", out_s)?;
        i.set_option("vo-image-jpeg-quality", "82")?;
        i.set_option("vo-image-jpeg-optimize", "0")?;
        i.set_option("vo-image-png-compression", "0")?;
        i.set_option("vf", vf)?;
        i.set_option("start", start.as_str())?;
        i.set_option("frames", 1i64)?;
        Ok(())
    })
    .ok()?;
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        return None;
    }
    let r = run_vo_image_after_load(&mut m, tmp, wait_secs);
    r
}

fn is_thumb_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("png")
            || e.eq_ignore_ascii_case("jpg")
            || e.eq_ignore_ascii_case("jpeg")
    })
}

fn first_image_in(dir: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_thumb_file(p))
        .collect();
    v.sort();
    v.into_iter().next()
}

/// First frame file from [vo=image] (``jpg`` default, or ``png`` / ``jpeg``).
fn pick_vo_out(dir: &Path) -> Option<PathBuf> {
    first_image_in(dir).or_else(|| {
        for name in [
            "00000001.jpg",
            "00000001.jpeg",
            "00000001.png",
        ] {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        None
    })
}

fn read_nonempty(src: &Path) -> Option<Vec<u8>> {
    let b = std::fs::read(src).ok()?;
    (!b.is_empty()).then_some(b)
}

/// Turn mpv [path] / [filename] into a local [PathBuf]. Rejects `http(s)://` etc. Accepts `file://`.
fn path_from_mpv_str(path_s: &str) -> Option<PathBuf> {
    let rest = if let Some(r) = path_s.strip_prefix("file://") {
        r.strip_prefix("localhost/")
            .or_else(|| r.strip_prefix("localhost"))
            .unwrap_or(r)
    } else if path_s.contains("://") {
        return None;
    } else {
        path_s
    };
    let can = std::fs::canonicalize(Path::new(rest)).ok()?;
    can.is_file().then_some(can)
}

/// Loaded local file, canonical, or `None` (idle, stream, or missing file).
pub(crate) fn local_file_from_mpv(mpv: &Mpv) -> Option<PathBuf> {
    let s = match mpv.get_property::<String>("path") {
        Ok(s) if !s.is_empty() => s,
        _ => match mpv.get_property::<String>("filename") {
            Ok(s) if !s.is_empty() => s,
            _ => return None,
        },
    };
    path_from_mpv_str(&s)
}

/// Store `duration` and `time-pos` in [crate::db] for the open local file. Use before switching
/// files or on close so the recent grid can show % without depending on watch_later text matching.
pub fn record_playback_for_current(mpv: &Mpv) {
    let Some(can) = local_file_from_mpv(mpv) else {
        return;
    };
    let d = mpv.get_property::<f64>("duration");
    let t = mpv.get_property::<f64>("time-pos");
    match (d, t) {
        (Ok(dur), Ok(pos))
            if dur.is_finite() && dur > 0.0 && pos.is_finite() && pos >= 0.0 =>
        {
            db::set_playback(&can, dur, pos);
        }
        (Ok(dur), _) if dur.is_finite() && dur > 0.0 => {
            db::set_duration(&can, dur);
        }
        _ => {}
    }
}

fn card_one(
    path: &Path,
    durs: &HashMap<String, f64>,
    tpos: &HashMap<String, f64>,
) -> CardData {
    if !path.exists() {
        return CardData {
            path: path.to_path_buf(),
            percent: 0.0,
            thumb: None,
            missing: true,
        };
    }
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_str();
    let st = s
        .and_then(|k| tpos.get(k).copied())
        .or_else(|| resume_start_seconds(&abs));
    let dur = s.and_then(|k| durs.get(k).copied());
    let pct = percent_from_resume(st, dur);
    let thumb = cached_thumbnail_for_display(&abs);
    CardData {
        path: abs,
        percent: pct,
        thumb,
        missing: false,
    }
}

/// Fills [CardData] for the recent grid. Loads duration + time-pos in two reads; run from an idle.
pub fn card_data_list(paths: &[PathBuf]) -> Vec<CardData> {
    let durs = db::load_duration_map();
    let tpos = db::load_time_pos_map();
    paths
        .iter()
        .map(|p| card_one(p, &durs, &tpos))
        .collect()
}
