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
fn watch_later_config_for(media: &Path) -> Option<PathBuf> {
    let dir = paths::watch_later()?;
    let can = std::fs::canonicalize(media).ok()?;
    let s = can.to_str()?;
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

fn resume_start_seconds(path: &Path) -> Option<f64> {
    let wl = watch_later_config_for(path)?;
    read_start_from_wl(&wl)
}

/// Near-end threshold (seconds) to show 100% progress.
const NEAR_END: f64 = 3.0;

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

/// Linear size divisor vs source frame (1/6 width & height) for [ensure_thumbnail] `vo=image` only.
const THUMB_SIZE_DIV: u32 = 6;

/// Quit-time `screenshot-to-file` thumbs: max edge fit inside this box, aspect ratio preserved, never upscaled.
const SCREENSHOT_MAX_W: u32 = 960;
const SCREENSHOT_MAX_H: u32 = 540;
const SCREENSHOT_JPEG_Q: u8 = 90;

/// Downsize quit-time screenshots to fit inside [SCREENSHOT_MAX_W]×[SCREENSHOT_MAX_H] when larger; Lanczos3 + JPEG. Returns [None] when already within bounds (caller keeps mpv’s bytes).
fn downscale_screenshot_raster(b: &[u8]) -> Option<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::imageops::FilterType;
    use image::ImageEncoder;
    let img = image::load_from_memory(b).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    let sw = w as f32;
    let sh = h as f32;
    let scale = (SCREENSHOT_MAX_W as f32 / sw)
        .min(SCREENSHOT_MAX_H as f32 / sh)
        .min(1.0f32);
    if scale >= 1.0 {
        return None;
    }
    let nw = ((sw * scale).round() as u32).clamp(1, SCREENSHOT_MAX_W);
    let nh = ((sh * scale).round() as u32).clamp(1, SCREENSHOT_MAX_H);
    let out = img.resize(nw, nh, FilterType::Lanczos3);
    let rgb = out.to_rgb8();
    let mut buf = vec![];
    JpegEncoder::new_with_quality(&mut buf, SCREENSHOT_JPEG_Q)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(buf)
}

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
fn db_thumb_for_canon_path(can: &Path) -> Option<Vec<u8>> {
    let s = can.to_str()?;
    let mtime = db::file_mtime_sec(can)?;
    db::take_thumb_if_current(s, mtime)
}

/// Current thumbnail for this path in [crate::db] when [db::file_mtime_sec] matches; **no libmpv** (use on the UI thread).
pub fn cached_thumbnail_for_path(path: &Path) -> Option<Vec<u8>> {
    if !path.exists() {
        return None;
    }
    let can = std::fs::canonicalize(path).ok()?;
    db_thumb_for_canon_path(&can)
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
    let b = run_libmpv_image_frame(&can, tag)?;
    const VO_IMAGE_START_SEC: f64 = 2.0;
    db::set_thumb(s, &b, mtime, VO_IMAGE_START_SEC);
    Some(b)
}

/// One frame via a short-lived [Mpv] with `vo=image` (writes a frame into a temp outdir; see `vo=image` in mpv’s `vo.rst`).
fn run_libmpv_image_frame(src: &Path, path_tag: u64) -> Option<Vec<u8>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let tmp = std::env::temp_dir().join(format!(
        "rhino-mpv-{}-{}",
        path_tag,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis()
    ));
    let out_s = tmp.to_str()?;
    let src_s = src.to_str()?;
    std::fs::create_dir_all(&tmp).ok()?;

    let mut m = Mpv::with_initializer(|i| {
        i.set_option("vo", "image")?;
        i.set_option("ao", "null")?;
        i.set_option("load-scripts", false)?;
        i.set_option("resume-playback", false)?;
        // Default VO is jpg; keep explicit + fast encode.
        i.set_option("vo-image-format", "jpg")?;
        i.set_option("vo-image-outdir", out_s)?;
        i.set_option("vo-image-jpeg-quality", "85")?;
        // High ``optimize`` can spend time on smaller files; 0 = faster writes.
        i.set_option("vo-image-jpeg-optimize", "0")?;
        i.set_option("vo-image-png-compression", "0")?;
        // 1/6 linear (≈1/36 pixels) for sharper on-card display; bilinear for looks.
        i.set_option(
            "vf",
            format!("scale=iw/{}:-2:flags=bilinear", THUMB_SIZE_DIV),
        )?;
        i.set_option("start", "2")?;
        i.set_option("frames", 1i64)?;
        Ok(())
    })
    .ok()?;
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        return None;
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut end_err = false;
    loop {
        if let Some(f) = pick_vo_out(&tmp) {
            if let Some(b) = read_nonempty(&f) {
                let _ = std::fs::remove_dir_all(&tmp);
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
            if let Some(f) = pick_vo_out(&tmp) {
                if let Some(b) = read_nonempty(&f) {
                    let _ = std::fs::remove_dir_all(&tmp);
                    return Some(b);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    None
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

fn save_thumb_to_cache(mpv: &Mpv, can: &Path) -> bool {
    let Some(s) = can.to_str() else {
        return false;
    };
    let mtime = match db::file_mtime_sec(can) {
        Some(m) => m,
        None => return false,
    };
    let t = mpv.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    if t.is_finite() && db::should_skip_quit_thumb(s, mtime, t) {
        return true;
    }
    let t_store = if t.is_finite() { t } else { 0.0 };
    let _ = mpv.set_property("screenshot-format", "jpeg");
    let _ = mpv.set_property("screenshot-jpeg-quality", 90i64);
    let _ = mpv.set_property("screenshot-png-compression", 0i64);
    let out = std::env::temp_dir().join(format!("rhino-qt-{}-{:x}.jpg", path_tag(s), s.len()));
    let Some(out_s) = out.to_str() else {
        return false;
    };
    if mpv.command("screenshot-to-file", &[out_s]).is_err() {
        return false;
    }
    let b = read_nonempty(&out);
    let _ = std::fs::remove_file(&out);
    let Some(raw) = b else {
        return false;
    };
    let img = downscale_screenshot_raster(&raw).unwrap_or(raw);
    db::set_thumb(s, &img, mtime, t_store);
    true
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

/// Screenshot of the current frame into the thumb cache. Heavy; call after the UI is responsive.
pub fn save_cached_thumb(mpv: &Mpv) {
    let Some(can) = local_file_from_mpv(mpv) else {
        return;
    };
    if !save_thumb_to_cache(mpv, &can) {
        // libmpv `screenshot-to-file` can fail; fill DB with a decoded frame (same as grid).
        let _ = ensure_thumbnail(&can);
    }
}

/// Screenshot and playback state for the open local file. Called on window close.
pub fn persist_on_quit(mpv: &Mpv) {
    save_cached_thumb(mpv);
    record_playback_for_current(mpv);
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
    let thumb = cached_thumbnail_for_path(&abs);
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
