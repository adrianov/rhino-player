/// After **`vf clr`** + **`restore_non_smooth_present_opts`**, **`seek`** to **`time-pos`** refreshes **`vo=libmpv`** without **`FileLoaded`**.
/// **Linux:** **`linux_ping_render_context`** (**`mpv_render_context_update`**); **macOS:** ping + **`mark_pending`**.
fn smooth_off_refresh_playhead(mpv: &Mpv, bundle: Option<&crate::mpv_embed::MpvBundle>) {
    let Ok(t) = mpv.get_property::<f64>("time-pos") else {
        return;
    };
    if !t.is_finite() || t < 0.0 {
        return;
    }
    let s = format!("{t:.4}");
    let _ = mpv.command("seek", &[s.as_str(), "absolute+exact"]);
    if video_log() {
        eprintln!("[rhino] video: (verbose) smooth-off playhead refresh seek");
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(b) = bundle {
        let _ = b.linux_ping_render_context();
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = bundle {
        b.macos_ping_render_context();
        b.macos_mark_display_pending();
    }
}
