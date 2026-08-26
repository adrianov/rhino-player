/// Max wait for chain-head duration / seek polling (corrupt VOB must not block grid workers long).
const VO_IMAGE_WAIT_CAP_SEC: u64 = 8;

fn vo_image_wait_chain_head(m: &mut Mpv, chapter: &Path, wait_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(VO_IMAGE_WAIT_CAP_SEC));
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
    let dur = vo_image_duration_sec(m);
    if crate::dvd_vob_timeline::chain_head_stretched(dur, seg) {
        let ifo = crate::dvd_vob_timeline::chain_head_ifo_local_from_mpv(pos, dur, seg);
        return ifo.is_finite() && (ifo - ifo_target).abs() < 2.0;
    }
    (pos - ifo_target).abs() < 2.0
}

fn vo_image_wait_demuxer(m: &mut Mpv, wait_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(VO_IMAGE_WAIT_CAP_SEC));
    loop {
        while m.wait_event(0.0).is_some() {}
        let dur = m.get_property::<f64>("duration").unwrap_or(f64::NAN);
        if dur.is_finite() && dur > 0.0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn vo_image_wait_seek(
    m: &mut Mpv,
    chapter: Option<&Path>,
    ifo_target: f64,
    mpv_target: f64,
    wait_secs: u64,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(VO_IMAGE_WAIT_CAP_SEC));
    loop {
        while m.wait_event(0.0).is_some() {}
        let ok = chapter
            .map(|ch| vo_image_at_ifo(m, ch, ifo_target))
            .unwrap_or_else(|| vo_image_pos_near(m, mpv_target));
        if ok {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

include!("thumb_vo_image_player.rs");

/// Map the IFO seek target onto mpv time, seek `absolute+exact`, wait out the seek and
/// the first decoded frame, then encode the screenshot WebP.
fn vo_image_capture_after_seek(
    m: &mut Mpv,
    src: &Path,
    ifo_seek: f64,
    chain_head: bool,
    dvd_vob: bool,
    wait_secs: u64,
) -> Option<Vec<u8>> {
    let mpv_t = crate::dvd_vob_timeline::preview_mpv_seek_sec(src, ifo_seek, m);
    if dvd_vob {
        vo_image_seek_log(
            src,
            format!("ifo={ifo_seek:.2} -> mpv={mpv_t:.2} chain={chain_head}"),
        );
    }
    vo_image_issue_exact_seek(m, src, mpv_t)?;
    let chapter = dvd_vob.then_some(src);
    vo_image_ensure_seeked(m, src, chapter, ifo_seek, mpv_t, wait_secs)?;
    if !vo_image_wait_frame(m, wait_secs) {
        eprintln!("[rhino] grid_thumb frame timeout {}", src.display());
        return None;
    }
    capture_screenshot_webp(m, wait_secs)
}

/// Issue the `absolute+exact` seek to the mapped mpv target; None on failure.
fn vo_image_issue_exact_seek(m: &mut Mpv, src: &Path, mpv_t: f64) -> Option<()> {
    let s = format!("{mpv_t:.3}");
    if m.command("seek", &[s.as_str(), "absolute+exact"]).is_err() {
        eprintln!(
            "[rhino] grid_thumb seek failed {} t={mpv_t:.2}",
            src.display()
        );
        return None;
    }
    Some(())
}

/// Wait out the exact seek; logs the settled raw position on timeout.
fn vo_image_ensure_seeked(
    m: &mut Mpv,
    src: &Path,
    chapter: Option<&Path>,
    ifo_seek: f64,
    mpv_t: f64,
    wait_secs: u64,
) -> Option<()> {
    if vo_image_wait_seek(m, chapter, ifo_seek, mpv_t, wait_secs) {
        return Some(());
    }
    let pos = m.get_property::<f64>("time-pos").unwrap_or(f64::NAN);
    eprintln!(
        "[rhino] grid_thumb seek timeout {} mpv={mpv_t:.2} pos={pos:.2}",
        src.display()
    );
    None
}

fn run_vo_image_one_frame(
    src: &Path,
    start_sec: f64,
    chapter_dur: f64,
    vf: &str,
    wait_secs: u64,
) -> Option<Vec<u8>> {
    let src_s = src.to_str()?;
    let dvd_vob = crate::video_ext::is_dvd_vob_path(src);
    let chain_head = dvd_vob && crate::dvd_vob_mpv_probe::is_title_chain_head(src);
    let cap = preview_cap_sec(chapter_dur, start_sec);
    let ifo_seek = crate::seek_bar_preview::cap_preview_seek_time(start_sec, cap);
    let mut m = vo_image_start(src, src_s, vf, ifo_seek, cap, chain_head, dvd_vob)?;
    vo_image_wait_loaded(&mut m, src, cap, chain_head, wait_secs)?;
    vo_image_capture_after_seek(&mut m, src, ifo_seek, chain_head, dvd_vob, wait_secs)
}

#[cfg(test)]
mod live_capture_tests {
    use super::*;
    use std::path::Path;

    /// `RHINO_GRID_THUMB_LIVE=1 cargo test live_vo_image_capture -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_vo_image_capture() {
        if std::env::var_os("RHINO_GRID_THUMB_LIVE").is_none() {
            return;
        }
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
        }
        let video = std::env::var("RHINO_GRID_THUMB_VIDEO")
            .unwrap_or_else(|_| "/home/crexus/Downloads/Orgy Palooza.mp4".into());
        let p = Path::new(&video);
        assert!(p.is_file(), "missing {video}");
        let b = run_vo_image_one_frame(
            p,
            2.0,
            0.0,
            "scale=640:-2:force_original_aspect_ratio=decrease:flags=bilinear",
            12,
        )
        .unwrap_or_else(|| panic!("capture failed for {video}"));
        assert!(
            crate::thumb_texture::thumb_webp_valid(&b),
            "bad webp head={:?}",
            &b[..b.len().min(12)]
        );
    }
}

/// VO configured and video dimensions known: a decoded frame can be captured.
fn vo_image_frame_ready(m: &Mpv) -> bool {
    let vo_ok = m.get_property::<bool>("vo-configured") == Ok(true);
    let sized = m
        .get_property::<i64>("dwidth")
        .ok()
        .zip(m.get_property::<i64>("dheight").ok())
        .is_some_and(|(w, h)| w > 0 && h > 0);
    vo_ok && sized
}

fn vo_image_wait_frame(m: &mut Mpv, wait_secs: u64) -> bool {
    if m.command("frame-step", &[] as &[&str]).is_err() {
        eprintln!("[rhino] grid_thumb frame-step failed");
    }
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(VO_IMAGE_WAIT_CAP_SEC));
    loop {
        while m.wait_event(0.0).is_some() {}
        if vo_image_frame_ready(m) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
