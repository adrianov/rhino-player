/// Always-on (throttled) A/V offset readout while the smooth **`vf`** is active, so lip-sync drift
/// is visible on plain `cargo run` without env flags. mpv **`avsync`** is audio-minus-video seconds.
pub(crate) fn log_smooth_avsync(mpv: &libmpv2::Mpv) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if !vf_chain_has_vapoursynth(mpv) || mpv.get_property::<bool>("pause").unwrap_or(true) {
        return;
    }
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let avsync = mpv.get_property::<f64>("avsync").ok();
    let pos = mpv.get_property::<f64>("time-pos").ok();
    let vf_fps = mpv.get_property::<f64>("estimated-vf-fps").ok();
    let display_fps = mpv.get_property::<f64>("display-fps").ok();
    let tag = match avsync {
        Some(a) if a.abs() > 0.08 => "DRIFT",
        _ => "ok",
    };
    eprintln!(
        "[rhino] video: avsync {tag} a-v={} time-pos={} vf-fps={} display-fps={}",
        avsync.map(|a| format!("{a:+.3}s")).unwrap_or_else(|| "?".into()),
        pos.map(|p| format!("{p:.2}")).unwrap_or_else(|| "?".into()),
        vf_fps.map(|f| format!("{f:.2}")).unwrap_or_else(|| "?".into()),
        display_fps.map(|f| format!("{f:.2}")).unwrap_or_else(|| "?".into()),
    );
}

/// Pause across a **`vf`** change so the chain reconfigures cleanly, then resume — **without** a
/// position **`seek`**. mpv keeps audio/video aligned by PTS through a live **`vf add`/`vf clr`**
/// (**`avsync`** stays ~0), so the old "A/V resync seek" only moved the picture (visible jump) and
/// never changed alignment. Position seeks are reserved for resume / user scrub / audio-track reopen.
pub(crate) struct VfAvSnap {
    was_playing: bool,
}

pub(crate) fn vf_av_pause_begin(mpv: &libmpv2::Mpv) -> VfAvSnap {
    let was_playing = !mpv.get_property::<bool>("pause").unwrap_or(true);
    if was_playing {
        let _ = mpv.set_property("pause", true);
    }
    VfAvSnap { was_playing }
}

pub(crate) fn vf_av_resume_end(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    snap: &VfAvSnap,
    tag: &str,
) {
    let state = if snap.was_playing { "playing" } else { "paused" };
    eprintln!("[rhino] video: {tag} vf change applied ({state}, no seek)");
    if snap.was_playing {
        let _ = mpv.set_property("pause", false);
    }
    vf_av_ping_render(bundle);
}

pub(crate) fn vf_av_ping_render(bundle: Option<&crate::mpv_embed::MpvBundle>) {
    #[cfg(not(target_os = "macos"))]
    if let Some(b) = bundle {
        b.linux_ping_render_context();
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = bundle {
        b.macos_ping_render_context();
        b.macos_mark_display_pending();
    }
}
