fn vo_image_drain(m: &mut Mpv) {
    while m.wait_event(0.0).is_some() {}
}

fn clear_vo_out_dir(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for e in read.flatten() {
        let p = e.path();
        if is_thumb_file(&p) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn vo_image_wait_duration(m: &mut Mpv, chapter_dur: f64) -> f64 {
    let ready = Instant::now() + Duration::from_secs(4);
    while Instant::now() < ready {
        vo_image_drain(m);
        if let Ok(d) = m.get_property::<f64>("duration") {
            if d.is_finite() && d > 0.0 {
                return if chapter_dur > 0.0 {
                    chapter_dur.min(d)
                } else {
                    d
                };
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    chapter_dur.max(0.0)
}

fn vo_image_wait_near_pos(m: &mut Mpv, target: f64) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        vo_image_drain(m);
        if let Ok(pos) = m.get_property::<f64>("time-pos") {
            if pos.is_finite() && (pos - target).abs() < 0.5 {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn screenshot_to_bytes(m: &mut Mpv, tmp: &Path) -> Option<Vec<u8>> {
    let out = tmp.join("rhino-cap.jpg");
    let _ = std::fs::remove_file(&out);
    let out_s = out.to_str()?;
    if m.command("screenshot-to-file", &[out_s, "video"]).is_err() {
        return None;
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        vo_image_drain(m);
        if let Some(b) = read_nonempty(&out) {
            return Some(b);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    read_nonempty(&out)
}

fn run_vo_image_one_frame(
    src: &Path,
    tmp: &Path,
    start_sec: f64,
    chapter_dur: f64,
    vf: &str,
    wait_secs: u64,
) -> Option<Vec<u8>> {
    let out_s = tmp.to_str()?;
    let src_s = src.to_str()?;
    std::fs::create_dir_all(tmp).ok()?;
    let dvd_vob = crate::video_ext::is_dvd_vob_path(src);
    let cap = if chapter_dur > 0.0 {
        chapter_dur
    } else {
        start_sec + 1.0
    };
    let seek_at = crate::seek_bar_preview::cap_preview_seek_time(start_sec, cap);
    let mut m = Mpv::with_initializer(|i| {
        i.set_option("vo", "image")?;
        i.set_option("ao", "null")?;
        let _ = i.set_option("keep-open", "always");
        let _ = i.set_option("vd-lavc-threads", "2");
        let _ = i.set_option("demuxer-readahead-secs", 0.0f64);
        let _ = i.set_option("demuxer-max-bytes", "128KiB");
        i.set_option("load-scripts", false)?;
        i.set_option("resume-playback", false)?;
        if dvd_vob {
            let _ = i.set_option("pause", true);
            let _ = i.set_option("hwdec", "auto");
            let _ = i.set_option("hr-seek", "yes");
        } else {
            let _ = i.set_option("hwdec", "no");
            let _ = i.set_option("hr-seek", false);
            let start = format!("{seek_at:.3}");
            i.set_option("start", start.as_str())?;
            i.set_option("frames", 1i64)?;
        }
        let _ = i.set_option("aid", "no");
        let _ = i.set_option("sid", "no");
        let _ = i.set_option("autoload-files", "no");
        let _ = i.set_option("audio-file-auto", "no");
        let _ = i.set_option("sub-auto", "no");
        i.set_option("vo-image-format", "jpg")?;
        i.set_option("vo-image-outdir", out_s)?;
        i.set_option("vo-image-jpeg-quality", "82")?;
        i.set_option("vf", vf)?;
        Ok(())
    })
    .ok()?;
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        return None;
    }
    if dvd_vob {
        let live_cap = vo_image_wait_duration(&mut m, chapter_dur);
        let cap = if live_cap > 0.0 { live_cap } else { cap };
        let t = crate::seek_bar_preview::cap_preview_seek_time(seek_at, cap);
        let _ = crate::seek_bar_preview::preview_run_seek(&m, t, true);
        vo_image_wait_near_pos(&mut m, t);
        clear_vo_out_dir(tmp);
        let _ = m.set_property("pause", false);
        if let Some(b) = screenshot_to_bytes(&mut m, tmp) {
            return Some(b);
        }
        let _ = m.set_property("frames", 1i64);
    }
    run_vo_image_after_load(&mut m, tmp, wait_secs)
}
