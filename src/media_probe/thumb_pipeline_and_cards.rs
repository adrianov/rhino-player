/// WebP in [crate::db] `media.thumb_webp`, rebuilt when the source file’s mtime changes.
/// Calls [run_libmpv_image_frame] on a **cache miss**; keep that work off the UI thread (see [crate::recent_view::schedule_thumb_backfill]).
pub fn ensure_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let entity = crate::playback_entity::db_path_for(path);
    let db_key = crate::db::history_key(&entity)?;
    let target = grid_thumb_target(&entity)?;
    if let Some(t) = db_thumb_for_entity_key(&db_key, &target.load, target.cache_time) {
        return Some(t);
    }
    let mtime = db::file_mtime_sec(&target.load)?;
    let b = run_libmpv_image_frame(&target.load, target.seek_sec, target.chapter_dur)?;
    if thumb_texture::thumb_webp_is_flat_fill(&b) {
        eprintln!(
            "[rhino] grid_thumb reject flat fill {}",
            target.load.display()
        );
        return None;
    }
    db::set_thumb(
        &db_key,
        &b,
        mtime,
        target.cache_time,
        target.load.to_str(),
    );
    Some(b)
}

/// Thumbnail: resume-position seek + small scale for continue cards.
fn run_libmpv_image_frame(src: &Path, start_sec: f64, chapter_dur: f64) -> Option<Vec<u8>> {
    run_vo_image_one_frame(
        src,
        start_sec,
        chapter_dur,
        &format!("scale={GRID_THUMB_W}:-2:force_original_aspect_ratio=decrease:flags=bilinear"),
        12,
    )
}

include!("thumb_screenshot_raw.rs");
include!("thumb_vo_image.rs");

/// Turn mpv [path] / [filename] into a local [PathBuf]. Rejects `http(s)://` etc. Accepts `file://`.
pub(crate) fn local_path_from_mpv_str(path_s: &str) -> Option<PathBuf> {
    let rest = if let Some(r) = path_s.strip_prefix("file://") {
        r.strip_prefix("localhost/")
            .or_else(|| r.strip_prefix("localhost"))
            .unwrap_or(r)
    } else if path_s.contains("://") {
        return None;
    } else {
        path_s
    };
    let raw = Path::new(rest);
    if let Ok(can) = std::fs::canonicalize(raw) {
        if can.is_file() {
            return Some(can);
        }
    }
    raw.is_file().then(|| raw.to_path_buf())
}

fn path_from_mpv_str(path_s: &str) -> Option<PathBuf> {
    local_path_from_mpv_str(path_s)
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
    let s = match mpv.get_property::<String>("path") {
        Ok(s) if !s.is_empty() => s,
        _ => match mpv.get_property::<String>("filename") {
            Ok(s) if !s.is_empty() => s,
            _ => return None,
        },
    };
    path_from_mpv_str(&s)
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
    CardData {
        path: std::fs::canonicalize(&entity).unwrap_or(entity),
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
