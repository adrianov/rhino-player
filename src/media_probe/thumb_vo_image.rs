fn vo_image_drain(m: &mut Mpv) {
    while m.wait_event(0.0).is_some() {}
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
            let _ = i.set_option("hwdec", "auto");
            let _ = i.set_option("hr-seek", "yes");
        } else {
            let _ = i.set_option("hwdec", "no");
            let _ = i.set_option("hr-seek", false);
        }
        let start = format!("{seek_at:.3}");
        i.set_option("start", start.as_str())?;
        i.set_option("frames", 1i64)?;
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
    if dvd_vob {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "grid_thumb vo=image {} start={seek_at:.2} cap={cap:.2}",
            src.display()
        ));
    }
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        return None;
    }
    run_vo_image_after_load(&mut m, tmp, wait_secs)
}
