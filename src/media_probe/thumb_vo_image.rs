fn vo_image_wait_chain_head(m: &mut Mpv, chapter: &Path, wait_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(wait_secs);
    loop {
        while m.wait_event(0.0).is_some() {}
        if crate::dvd_vob_timeline::chain_head_mpv_ready(chapter, m) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn vo_image_at_ifo(m: &Mpv, chapter: &Path, ifo_target: f64) -> bool {
    let pos = m.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    if !pos.is_finite() {
        return false;
    }
    let Some(seg) = crate::dvd_vob_timeline::chain_head_ifo_seg(chapter) else {
        return (pos - ifo_target).abs() < 2.0;
    };
    let dur = m
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    if crate::dvd_vob_timeline::chain_head_stretched(dur, seg) {
        let ifo = crate::dvd_vob_timeline::chain_head_ifo_local_from_mpv(pos, dur, seg);
        return ifo.is_finite() && (ifo - ifo_target).abs() < 2.0;
    }
    (pos - ifo_target).abs() < 2.0
}

fn vo_image_wait_seek(m: &mut Mpv, chapter: Option<&Path>, ifo_target: f64, mpv_target: f64, wait_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(8));
    loop {
        while m.wait_event(0.0).is_some() {}
        let ok = chapter
            .map(|ch| vo_image_at_ifo(m, ch, ifo_target))
            .unwrap_or_else(|| {
                let pos = m.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
                pos.is_finite() && (pos - mpv_target).abs() < 2.0
            });
        if ok {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn purge_vo_image_out(tmp: &Path) {
    let Ok(rd) = std::fs::read_dir(tmp) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| {
            x.eq_ignore_ascii_case("jpg")
                || x.eq_ignore_ascii_case("jpeg")
                || x.eq_ignore_ascii_case("png")
        }) {
            let _ = std::fs::remove_file(p);
        }
    }
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
    let chain_head = dvd_vob && crate::dvd_vob_mpv_probe::is_title_chain_head(src);
    let cap = if chapter_dur > 0.0 {
        chapter_dur
    } else {
        start_sec + 1.0
    };
    let ifo_seek = crate::seek_bar_preview::cap_preview_seek_time(start_sec, cap);
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
            // Match seek preview: software decode on DVD `.vob` (hwdec breaks vo paths on macOS).
            let _ = i.set_option("hwdec", "no");
            let _ = i.set_option("hr-seek", "yes");
        } else {
            let _ = i.set_option("hwdec", "no");
            let _ = i.set_option("hr-seek", false);
        }
        if chain_head {
            let _ = i.set_option("pause", true);
        } else {
            let start = format!("{ifo_seek:.3}");
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
    if dvd_vob {
        crate::dvd_vob_log::dvd_seek_log(format!(
            "grid_thumb vo=image {} ifo={ifo_seek:.2} cap={cap:.2} chain={chain_head}",
            src.display()
        ));
    }
    if m.command("loadfile", &[src_s, "replace"]).is_err() {
        return None;
    }
    if chain_head {
        let ifo_seg = crate::dvd_vob_timeline::chain_head_ifo_seg(src).unwrap_or(cap);
        if !vo_image_wait_chain_head(&mut m, src, wait_secs) {
            let mpv_dur = m
                .get_property::<f64>("duration")
                .ok()
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(0.0);
            if crate::dvd_vob_timeline::chain_head_stretched(mpv_dur, ifo_seg) {
                crate::dvd_vob_log::dvd_seek_log(format!(
                    "grid_thumb vo=image {} chain-head duration timeout",
                    src.display()
                ));
                return None;
            }
            crate::dvd_vob_log::dvd_seek_log(format!(
                "grid_thumb vo=image {} chain-head natural dur={mpv_dur:.2} ifo={ifo_seg:.2}",
                src.display()
            ));
        }
        let mpv_t = crate::dvd_vob_timeline::preview_mpv_seek_sec(src, ifo_seek, &m);
        crate::dvd_vob_log::dvd_seek_log(format!(
            "grid_thumb vo=image {} ifo={ifo_seek:.2} -> mpv={mpv_t:.2}",
            src.display()
        ));
        let s = format!("{mpv_t:.3}");
        if m.command("seek", &[s.as_str(), "absolute+exact"]).is_err() {
            return None;
        }
        if !vo_image_wait_seek(&mut m, Some(src), ifo_seek, mpv_t, wait_secs) {
            let pos = m.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
            crate::dvd_vob_log::dvd_seek_log(format!(
                "grid_thumb vo=image {} seek timeout mpv={mpv_t:.2} pos={pos:.2}",
                src.display()
            ));
            return None;
        }
        purge_vo_image_out(tmp);
        let _ = m.set_property("pause", false);
        let _ = m.set_property("frames", 1i64);
        while m.wait_event(0.0).is_some() {}
    }
    run_vo_image_after_load(&mut m, tmp, wait_secs)
}
