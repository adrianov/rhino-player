/// After **`vf clr`**/**`vf add`**, **`seek`** to **`time-pos`** re-aligns A/V without **`FileLoaded`**.
/// **Linux:** **`linux_ping_render_context`**; **macOS:** ping + **`mark_pending`**.
fn refresh_playhead_after_vf_change(
    mpv: &Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    tag: &str,
) {
    let Ok(t) = mpv.get_property::<f64>("time-pos") else {
        return;
    };
    if !t.is_finite() || t < 0.0 {
        return;
    }
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    if video_log() {
        eprintln!("[rhino] video: (verbose) {tag} playhead refresh seek");
    }
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

fn smooth_off_refresh_playhead(mpv: &Mpv, bundle: Option<&crate::mpv_embed::MpvBundle>) {
    refresh_playhead_after_vf_change(mpv, bundle, "smooth-off");
}

/// User picked another audio stream while the smooth motion filter graph is active. The reopened
/// audio decoder can drift from the buffered FlowFPS frame queue; an exact seek to the current
/// position realigns A/V without rebuilding the graph. Only the buffering `vapoursynth` vf needs
/// this — plain `display-resample` keeps audio synced on its own, and load / chapter restore set
/// the track before unpause.
pub(crate) fn resync_av_after_audio_track_change(b: &crate::mpv_embed::MpvBundle) {
    let mpv = &b.mpv;
    if !vf_chain_has_vapoursynth(mpv) || mpv.get_property::<bool>("pause").unwrap_or(true) {
        return;
    }
    refresh_playhead_after_vf_change(mpv, Some(b), "audio-track");
}
