/// Grid still outcome. [Self::Unparseable] means load/demux failed — drop the catalog path.
#[derive(Debug)]
pub enum GridThumb {
    Ready,
    Unparseable,
    Miss,
}

enum ThumbFail {
    Unparseable,
    Other,
}

/// WebP in [crate::db] `media.thumb_webp`, rebuilt when the source file’s mtime changes.
/// Calls [run_libmpv_image_frame] on a **cache miss**; keep that work off the UI thread (see [crate::recent_view::schedule_thumb_backfill]).
pub fn ensure_thumbnail(path: &Path) -> GridThumb {
    let entity = crate::playback_entity::db_path_for(path);
    let Some(db_key) = crate::db::history_key(&entity) else {
        return GridThumb::Miss;
    };
    let Some(target) = grid_thumb_target(&entity) else {
        return GridThumb::Miss;
    };
    if db_thumb_for_entity_key(&db_key, &target.load, target.cache_time).is_some() {
        return GridThumb::Ready;
    }
    // Resume still at start but a still exists — keep it (do not re-seek to the 2s fallback).
    if stored_thumb_while_at_start(path).is_some() {
        return GridThumb::Ready;
    }
    let Some(mtime) = db::file_mtime_sec(&target.load) else {
        return GridThumb::Miss;
    };
    match run_libmpv_image_frame(
        &target.load,
        target.seek_sec,
        target.chapter_dur,
        target.keyframes,
    ) {
        Ok(b) => {
            persist_grid_thumb(&db_key, &b, mtime, &target);
            GridThumb::Ready
        }
        Err(ThumbFail::Unparseable) => forget_unparseable(path),
        Err(ThumbFail::Other) => GridThumb::Miss,
    }
}

fn forget_unparseable(path: &Path) -> GridThumb {
    if !should_forget_unparseable(path) {
        return GridThumb::Miss;
    }
    eprintln!("[rhino] catalog: drop unparseable {}", path.display());
    drop_catalog_path(path);
    GridThumb::Unparseable
}

/// Listed strip path → still outcome. Forgets catalog when the path is absent (feature 34).
pub(crate) fn ensure_listed_thumbnail(path: &Path) -> (GridThumb, PathBuf) {
    if !path.exists() {
        let _ = forget_missing(path);
        return (GridThumb::Unparseable, path.to_path_buf());
    }
    let can = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if thumb_backfill_satisfied(&can) {
        return (GridThumb::Miss, can);
    }
    (ensure_thumbnail(&can), can)
}

/// Write the captured still; mark flat fills so a second worker pass will accept them.
fn persist_grid_thumb(db_key: &str, b: &[u8], mtime: i64, target: &GridThumbTarget) {
    db::set_thumb(db_key, b, mtime, target.cache_time, target.load.to_str());
    if crate::thumb_texture::thumb_webp_is_flat_fill(b) {
        grid_thumb_flat_capture::mark_done(db_key);
    }
}

/// Thumbnail: resume-position seek + small scale for continue cards.
/// Unstarted titles use a keyframe seek so Lucky / first-open stills land faster.
fn run_libmpv_image_frame(
    src: &Path,
    start_sec: f64,
    chapter_dur: f64,
    keyframes: bool,
) -> Result<Vec<u8>, ThumbFail> {
    run_vo_image_one_frame(
        src,
        start_sec,
        chapter_dur,
        &format!("scale={GRID_THUMB_W}:-2:force_original_aspect_ratio=decrease:flags=bilinear"),
        12,
        keyframes,
    )
}

include!("thumb_screenshot_raw.rs");
include!("thumb_vo_image.rs");

/// Read mpv [path] / [filename] as a local [PathBuf] without touching the filesystem.
/// Accepts `file://`; rejects `http(s)://` and every other scheme.
fn local_path_no_stat(path_s: &str) -> Option<PathBuf> {
    let rest = if let Some(r) = path_s.strip_prefix("file://") {
        r.strip_prefix("localhost/")
            .or_else(|| r.strip_prefix("localhost"))
            .unwrap_or(r)
    } else if path_s.contains("://") {
        return None;
    } else {
        path_s
    };
    Some(PathBuf::from(rest))
}

/// Like [local_path_no_stat], but only while the file is still present; canonical form when available.
pub(crate) fn local_path_from_mpv_str(path_s: &str) -> Option<PathBuf> {
    let raw = local_path_no_stat(path_s)?;
    if let Ok(can) = std::fs::canonicalize(&raw) {
        if can.is_file() {
            return Some(can);
        }
    }
    raw.is_file().then_some(raw)
}

/// mpv `path`, else `filename`; `None` while mpv sits idle.
fn mpv_path_str(mpv: &Mpv) -> Option<String> {
    ["path", "filename"]
        .iter()
        .filter_map(|k| mpv.get_property::<String>(k).ok())
        .find(|s| !s.is_empty())
}

/// The media item mpv holds, under the name it was opened with — still reported after that name
/// disappears (a download renamed on completion, a file moved or deleted mid-playback). Answers
/// “is something on screen?”; read or store the path through [shell_media_path] instead.
/// `None` while mpv is idle or pulling a network stream.
pub(crate) fn open_media_path(mpv: &Mpv, shell: Option<&std::path::Path>) -> Option<PathBuf> {
    mpv_path_str(mpv)
        .and_then(|s| local_path_no_stat(&s))
        .or_else(|| shell.map(std::path::Path::to_path_buf))
}

#[cfg(test)]
mod open_media_path_tests {
    use super::local_path_no_stat;
    use std::path::Path;

    /// A finished download is renamed under mpv; the name it was opened with must still parse.
    #[test]
    fn keeps_vanished_local_path_and_skips_streams() {
        let gone = "/dl/clip.mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp";
        assert_eq!(local_path_no_stat(gone).as_deref(), Some(Path::new(gone)));
        assert_eq!(
            local_path_no_stat("file:///dl/clip.mkv").as_deref(),
            Some(Path::new("/dl/clip.mkv"))
        );
        assert!(local_path_no_stat("https://host/clip.mkv").is_none());
        assert!(local_path_no_stat("bd://0").is_none());
    }
}

#[cfg(test)]
mod unparseable_forget_tests {
    use super::should_forget_unparseable;
    use std::path::Path;

    #[test]
    fn forgets_ordinary_file() {
        assert!(should_forget_unparseable(Path::new("/store/broken.mkv")));
    }

    #[test]
    fn keeps_incomplete_download() {
        assert!(!should_forget_unparseable(Path::new(
            "/dl/clip.mkv.RSRXEZ4AWN67MGBANBT6YLR32JW32GVZSZLYN2Y.dctmp"
        )));
    }

    #[test]
    fn keeps_dvd_chapter_vob() {
        assert!(!should_forget_unparseable(Path::new(
            "/disc/VIDEO_TS/VTS_01_1.VOB"
        )));
    }
}

/// Local filesystem path for the open item: mpv `path` when it is a file, else the shell path
/// ([`crate::mpv_embed::MpvBundle::me_budget_shell_path`]) for `bd://` / disc trees.
pub(crate) fn shell_media_path(mpv: &Mpv, shell: Option<&std::path::Path>) -> Option<PathBuf> {
    if let Some(p) = local_file_from_mpv(mpv) {
        return Some(p);
    }
    shell
        .and_then(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.to_path_buf())))
        .filter(|p| p.exists())
}

/// True when mpv reports a finite, non-zero duration (demuxer ready enough to seek).
pub(crate) fn mpv_has_known_duration(mpv: &Mpv) -> bool {
    mpv.get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .is_some()
}

/// Local path mpv is actually decoding (never the shell intent cell used during continue hover).
pub(crate) fn mpv_local_open_path(mpv: &Mpv) -> Option<PathBuf> {
    local_file_from_mpv(mpv)
}

/// True when mpv's open item is the same title as `path` (local file or disc root).
pub(crate) fn mpv_matches_open_target(
    mpv: &Mpv,
    shell: Option<&std::path::Path>,
    path: &std::path::Path,
) -> bool {
    let Some(open) = shell_media_path(mpv, shell) else {
        return false;
    };
    let want = crate::video_ext::resolve_open_media_path(path);
    crate::video_ext::paths_same_file(&open, &want)
}

/// Warm hit: mpv already decodes this exact local target with known duration.
/// Never uses [me_budget_shell_path] — hover sets that before `loadfile` and would match the wrong title while mpv still holds the previous disc (`bd://`, …).
pub(crate) fn mpv_warm_hit_ready(mpv: &Mpv, path: &std::path::Path) -> bool {
    if !mpv_has_known_duration(mpv) {
        return false;
    }
    let Some(open) = mpv_local_open_path(mpv) else {
        return false;
    };
    let want = crate::video_ext::resolve_open_media_path(path);
    crate::video_ext::paths_same_file(&open, &want)
}

/// Loaded local file, canonical, or `None` (idle, stream, or missing file).
pub(crate) fn local_file_from_mpv(mpv: &Mpv) -> Option<PathBuf> {
    mpv_path_str(mpv).and_then(|s| local_path_from_mpv_str(&s))
}

/// Store `duration` and `time-pos` in [crate::db] for the open item. Use before switching
/// media or on close so the recent grid can show %. Pass [shell_media_path]'s `shell` when mpv
/// reports `bd://` (Blu-ray) instead of a filesystem path.
pub fn record_playback_for_current(
    mpv: &Mpv,
    shell: Option<&std::path::Path>,
    transport_bar: Option<(f64, f64)>,
) {
    crate::playback_entity::persist_from_mpv(mpv, shell, transport_bar);
}

fn card_one(path: &Path, durs: &HashMap<String, f64>, tpos: &HashMap<String, f64>) -> CardData {
    if !path.exists() {
        let _ = forget_missing(path);
        return CardData {
            path: path.to_path_buf(),
            percent: 0.0,
            thumb: None,
            missing: true,
            resume_sec: 0.0,
            duration_sec: 0.0,
        };
    }
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let entity = crate::playback_entity::db_path_for(&abs);
    let (resume, duration) = crate::playback_entity::card_resume_duration(&entity, durs, tpos);
    let pct = percent_from_resume(Some(resume), Some(duration));
    let thumb = cached_thumbnail_for_display(&entity);
    // Strip identity stays the listing path; canonicalize is only for resume / stills.
    CardData {
        path: path.to_path_buf(),
        percent: pct,
        thumb,
        missing: false,
        resume_sec: resume,
        duration_sec: duration,
    }
}

/// Fills [CardData] for the recent grid. Loads duration + time-pos in two reads; run from an idle.
pub fn card_data_list(paths: &[PathBuf]) -> Vec<CardData> {
    let durs = db::load_duration_map();
    let tpos = db::load_time_pos_map();
    paths.iter().map(|p| card_one(p, &durs, &tpos)).collect()
}

#[cfg(test)]
mod card_data_path_tests {
    use super::card_data_list;
    use std::path::PathBuf;

    #[test]
    fn card_data_keeps_strip_path() {
        let p = std::env::temp_dir().join(format!("rhino-card-path-{}.bin", std::process::id()));
        std::fs::write(&p, [0u8; 1]).unwrap();
        let listed = card_data_list(std::slice::from_ref(&p));
        std::fs::remove_file(&p).ok();
        assert_eq!(listed[0].path, p);
    }

    #[test]
    fn card_data_missing_keeps_given_path() {
        let p = PathBuf::from("/no/such/rhino-card-path-missing.mkv");
        let listed = card_data_list(std::slice::from_ref(&p));
        assert!(listed[0].missing);
        assert_eq!(listed[0].path, p);
    }
}
