/// PNG in [crate::db] `media.thumb_png`, rebuilt when the source file’s mtime changes.
/// Calls [run_libmpv_image_frame] on a **cache miss**; keep that work off the UI thread (see [crate::recent_view::schedule_thumb_backfill]).
pub fn ensure_thumbnail(path: &Path) -> Option<Vec<u8>> {
    let entity = crate::playback_entity::db_path_for(path);
    let db_key = crate::db::history_key(&entity)?;
    let target = grid_thumb_target(&entity)?;
    if let Some(t) = db_thumb_for_entity_key(&db_key, &target.load, target.cache_time) {
        return Some(t);
    }
    let mtime = db::file_mtime_sec(&target.load)?;
    let b = run_libmpv_image_frame(&target.load, path_tag(&db_key), target.seek_sec)?;
    db::set_thumb(&db_key, &b, mtime, target.cache_time);
    Some(b)
}

/// One `vo=image` still (writes into [tmp_dir]), with [loadfile] already applied by the caller, or
/// shared setup through [run_vo_image_one_frame].
fn run_vo_image_after_load(m: &mut Mpv, tmp: &Path, deadline_secs: u64) -> Option<Vec<u8>> {
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
        i.set_option("vf", vf)?;
        i.set_option("start", start.as_str())?;
        i.set_option("frames", 1i64)?;
        Ok(())
    })
    .ok()?;
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        return None;
    }
    run_vo_image_after_load(&mut m, tmp, wait_secs)
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
        for name in ["00000001.jpg", "00000001.jpeg", "00000001.png"] {
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
pub fn record_playback_for_current(mpv: &Mpv, shell: Option<&std::path::Path>) {
    crate::playback_entity::persist_from_mpv(mpv, shell);
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
