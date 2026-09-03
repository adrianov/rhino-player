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

fn vo_image_wait_demuxer(m: &mut Mpv, wait_secs: u64) -> Result<(), ThumbFail> {
    let deadline = Instant::now() + Duration::from_secs(wait_secs.min(VO_IMAGE_WAIT_CAP_SEC));
    loop {
        drain_thumb_events(m)?;
        let dur = m.get_property::<f64>("duration").unwrap_or(f64::NAN);
        if dur.is_finite() && dur > 0.0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ThumbFail::Other);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Engine-reported load/demux failure only — a wait timeout is not unparseable.
fn drain_thumb_events(m: &mut Mpv) -> Result<(), ThumbFail> {
    while let Some(ev) = m.wait_event(0.0) {
        match ev {
            Ok(libmpv2::events::Event::EndFile(r))
                if r == libmpv2::mpv_end_file_reason::Error =>
            {
                return Err(ThumbFail::Unparseable);
            }
            Err(e) if thumb_wait_is_load_fail(&e) => return Err(ThumbFail::Unparseable),
            _ => {}
        }
    }
    Ok(())
}

fn thumb_wait_is_load_fail(err: &libmpv2::Error) -> bool {
    matches!(
        err,
        libmpv2::Error::Raw(
            libmpv2::mpv_error::LoadingFailed
                | libmpv2::mpv_error::NothingToPlay
                | libmpv2::mpv_error::UnknownFormat
        )
    )
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

/// Map the IFO seek target onto mpv time, seek, wait out the first decoded frame, then encode WebP.
/// Resume-position stills use `absolute+exact`; unstarted / start-fallback stills use keyframes.
fn vo_image_capture_after_seek(
    m: &mut Mpv,
    src: &Path,
    ifo_seek: f64,
    chain_head: bool,
    dvd_vob: bool,
    wait_secs: u64,
    keyframes: bool,
) -> Option<Vec<u8>> {
    let mpv_t = crate::dvd_vob_timeline::preview_mpv_seek_sec(src, ifo_seek, m);
    if dvd_vob {
        vo_image_seek_log(
            src,
            format!("ifo={ifo_seek:.2} -> mpv={mpv_t:.2} chain={chain_head}"),
        );
    }
    vo_image_issue_seek(m, src, mpv_t, keyframes)?;
    if !keyframes {
        let chapter = dvd_vob.then_some(src);
        vo_image_ensure_seeked(m, src, chapter, ifo_seek, mpv_t, wait_secs)?;
    }
    if !vo_image_wait_frame(m, wait_secs) {
        eprintln!("[rhino] grid_thumb frame timeout {}", src.display());
        return None;
    }
    // Same cap as seek / demuxer waits — flat/dark stability exits earlier when possible.
    capture_screenshot_webp(m, wait_secs.min(VO_IMAGE_WAIT_CAP_SEC))
}

/// Issue the mapped mpv seek; None on failure.
fn vo_image_issue_seek(m: &mut Mpv, src: &Path, mpv_t: f64, keyframes: bool) -> Option<()> {
    let s = format!("{mpv_t:.3}");
    let mode = if keyframes {
        "absolute+keyframes"
    } else {
        "absolute+exact"
    };
    if m.command("seek", &[s.as_str(), mode]).is_err() {
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

#[path = "thumb_vo_image_flat_nudge.rs"]
mod thumb_vo_image_flat_nudge;
use thumb_vo_image_flat_nudge::{vo_image_prefer_nonflat, FlatNudgeCtx};

struct VoImagePlan {
    dvd_vob: bool,
    chain_head: bool,
    cap: f64,
    ifo_seek: f64,
}

fn vo_image_plan(src: &Path, start_sec: f64, chapter_dur: f64) -> VoImagePlan {
    let dvd_vob = crate::video_ext::is_dvd_vob_path(src);
    let chain_head = dvd_vob && crate::dvd_vob_mpv_probe::is_title_chain_head(src);
    let cap = preview_cap_sec(chapter_dur, start_sec);
    let ifo_seek = crate::seek_bar_preview::cap_preview_seek_time(start_sec, cap);
    VoImagePlan {
        dvd_vob,
        chain_head,
        cap,
        ifo_seek,
    }
}

fn run_vo_image_one_frame(
    src: &Path,
    start_sec: f64,
    chapter_dur: f64,
    vf: &str,
    wait_secs: u64,
    keyframes: bool,
) -> Result<Vec<u8>, ThumbFail> {
    let src_s = src.to_str().ok_or(ThumbFail::Other)?;
    let plan = vo_image_plan(src, start_sec, chapter_dur);
    eprintln!(
        "[rhino] grid_thumb: creating {} seek={:.2}",
        src.display(),
        plan.ifo_seek
    );
    thumb_src_set(src);
    let _src_guard = ThumbSrcGuard;
    let mut m = vo_image_start(
        src,
        src_s,
        vf,
        plan.ifo_seek,
        plan.cap,
        plan.chain_head,
        plan.dvd_vob,
    )?;
    vo_image_wait_loaded(&mut m, src, plan.cap, plan.chain_head, wait_secs)?;
    vo_image_grab_frame(&mut m, src, &plan, wait_secs, keyframes)
}

fn vo_image_grab_frame(
    m: &mut Mpv,
    src: &Path,
    plan: &VoImagePlan,
    wait_secs: u64,
    keyframes: bool,
) -> Result<Vec<u8>, ThumbFail> {
    let first = vo_image_capture_after_seek(
        m,
        src,
        plan.ifo_seek,
        plan.chain_head,
        plan.dvd_vob,
        wait_secs,
        keyframes,
    )
    .ok_or(ThumbFail::Other)?;
    let cap = vo_image_probe_cap(m, plan);
    vo_image_prefer_nonflat(
        FlatNudgeCtx {
            m,
            src,
            ifo_seek: plan.ifo_seek,
            cap,
            chain_head: plan.chain_head,
            dvd_vob: plan.dvd_vob,
            wait_secs,
        },
        first,
    )
    .ok_or(ThumbFail::Other)
}

/// DVD chapters stay on the planned chapter cap; other files use demuxer duration after load.
fn vo_image_probe_cap(m: &Mpv, plan: &VoImagePlan) -> f64 {
    if plan.dvd_vob {
        return plan.cap;
    }
    vo_image_duration_sec(m).max(plan.cap)
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
            true,
        )
        .unwrap_or_else(|_| panic!("capture failed for {video}"));
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
