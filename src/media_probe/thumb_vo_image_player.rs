// VO-image thumbnail player lifecycle (include!'d from `thumb_vo_image.rs`):
// spawn options, loadfile start, and the loaded-source wait.

/// Latest finite positive mpv duration, else 0.0.
fn vo_image_duration_sec(m: &Mpv) -> f64 {
    m.get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0)
}

/// True when the raw mpv position sits within tolerance of `target`.
fn vo_image_pos_near(m: &Mpv, target: f64) -> bool {
    let pos = m.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    pos.is_finite() && (pos - target).abs() < 2.0
}

/// Seek cap for previews: chapter length when known, else just past the start.
fn preview_cap_sec(chapter_dur: f64, start_sec: f64) -> f64 {
    if chapter_dur > 0.0 {
        chapter_dur
    } else {
        start_sec + 1.0
    }
}

/// Shared grid_thumb DVD seek log prefix.
fn vo_image_seek_log(src: &Path, msg: String) {
    crate::dvd_vob_log::dvd_seek_log(format!("grid_thumb {} {msg}", src.display()));
}

/// Paused null-VO software-decode player tuned for single-frame thumbnail capture.
fn vo_image_spawn(vf: &str) -> Option<Mpv> {
    Mpv::with_initializer(|i| {
        i.set_option("vo", "null")?;
        i.set_option("ao", "null")?;
        // Software decode + hr-seek: load then `absolute+exact` (not `--start`, which snaps to keyframes).
        i.set_option("vf", vf)?;
        // Best-effort tuning; a rejected option never blocks thumbnail capture.
        for (key, val) in [
            ("hwdec", "no"),
            ("hr-seek", "yes"),
            ("pause", "yes"),
            ("keep-open", "always"),
            ("vd-lavc-threads", "2"),
            ("demuxer-readahead-secs", "0"),
            ("demuxer-max-bytes", "128KiB"),
            ("aid", "no"),
            ("sid", "no"),
            ("autoload-files", "no"),
            ("audio-file-auto", "no"),
            ("sub-auto", "no"),
        ] {
            let _ = i.set_option(key, val);
        }
        i.set_option("load-scripts", false)?;
        i.set_option("resume-playback", false)?;
        Ok(())
    })
    .ok()
}

/// Spawn, log the DVD seek plan, and loadfile. Load failure is unparseable; spawn failure is not.
fn vo_image_start(
    src: &Path,
    src_s: &str,
    vf: &str,
    ifo_seek: f64,
    cap: f64,
    chain_head: bool,
    dvd_vob: bool,
) -> Result<Mpv, ThumbFail> {
    let m = vo_image_spawn(vf).ok_or(ThumbFail::Other)?;
    if dvd_vob {
        vo_image_seek_log(
            src,
            format!("ifo={ifo_seek:.2} cap={cap:.2} chain={chain_head}"),
        );
    }
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        eprintln!("[rhino] grid_thumb loadfile failed {}", src.display());
        return Err(ThumbFail::Unparseable);
    }
    Ok(m)
}

/// Wait until the loaded source is playable: chain-head duration mapping or plain demuxer ready.
fn vo_image_wait_loaded(
    m: &mut Mpv,
    src: &Path,
    cap: f64,
    chain_head: bool,
    wait_secs: u64,
) -> Result<(), ThumbFail> {
    if chain_head {
        let ifo_seg = crate::dvd_vob_timeline::chain_head_ifo_seg(src).unwrap_or(cap);
        if !vo_image_wait_chain_head(m, src, wait_secs) {
            let mpv_dur = vo_image_duration_sec(m);
            if crate::dvd_vob_timeline::chain_head_stretched(mpv_dur, ifo_seg) {
                vo_image_seek_log(src, "chain-head duration timeout".into());
                return Err(ThumbFail::Other);
            }
            vo_image_seek_log(
                src,
                format!("chain-head natural dur={mpv_dur:.2} ifo={ifo_seg:.2}"),
            );
        }
    } else if let Err(e) = vo_image_wait_demuxer(m, wait_secs) {
        log_demux_wait(src, &e);
        return Err(e);
    }
    Ok(())
}

fn log_demux_wait(src: &Path, e: &ThumbFail) {
    match e {
        ThumbFail::Unparseable => {
            eprintln!("[rhino] grid_thumb demux failed {}", src.display());
        }
        ThumbFail::Other => {
            eprintln!("[rhino] grid_thumb demuxer timeout {}", src.display());
        }
    }
}
