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

pub(crate) fn smooth_on_refresh_playhead(mpv: &Mpv, bundle: Option<&crate::mpv_embed::MpvBundle>) {
    refresh_playhead_after_vf_change(mpv, bundle, "smooth-on");
}
