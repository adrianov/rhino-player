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
    let t = thumb_time_for_path(s);
    let b = run_libmpv_image_frame(&can, tag, t)?;
    db::set_thumb(s, &b, mtime, t);
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
/// files or on close so the recent grid can show %.
pub fn record_playback_for_current(mpv: &Mpv) {
    let Some(can) = local_file_from_mpv(mpv) else {
        return;
    };
    let d = mpv.get_property::<f64>("duration");
    let t = mpv.get_property::<f64>("time-pos");
    match (d, t) {
        (Ok(dur), Ok(pos)) if dur.is_finite() && dur > 0.0 && pos.is_finite() && pos >= 0.0 => {
            db::set_playback(&can, dur, pos);
        }
        (Ok(dur), _) if dur.is_finite() && dur > 0.0 => {
            db::set_duration(&can, dur);
        }
        _ => {}
    }
}

fn card_one(path: &Path, durs: &HashMap<String, f64>, tpos: &HashMap<String, f64>) -> CardData {
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
    let st = s.and_then(|k| tpos.get(k).copied());
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
    paths.iter().map(|p| card_one(p, &durs, &tpos)).collect()
}
