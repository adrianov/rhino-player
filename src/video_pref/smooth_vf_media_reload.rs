// Same-media loadfile replace when mpv rejects vf add vapoursynth after vf remove.

/// Reload the open file at the current playhead so mpv can attach vapoursynth again.
pub(crate) fn reload_open_media_for_vf_reset(b: &MpvBundle, resume_playing: bool) -> bool {
    let Some(path) = crate::media_probe::local_file_from_mpv(&b.mpv) else {
        eprintln!("[rhino] video: vf reset reload skipped (no local path)");
        return false;
    };
    let pos = match b.mpv.get_property::<f64>("time-pos") {
        Ok(p) if p.is_finite() && p >= 0.0 => p,
        _ => {
            eprintln!("[rhino] video: vf reset reload skipped (no playhead)");
            return false;
        }
    };
    strip_vapoursynth_before_replace_media(b);
    let _ = b.mpv.command("stop", &[]);
    let resume = if pos > 0.05 { Some(pos) } else { None };
    match b.load_file_path(&path, false, false, false, resume) {
        Ok(()) => {
            eprintln!(
                "[rhino] video: stop+loadfile replace for vapoursynth reattach path={} pos={pos:.2} resume_playing={resume_playing}",
                path.display()
            );
            if !resume_playing {
                let _ = b.mpv.set_property("pause", true);
            }
            crate::app::transport_drain_after_loadfile_idle();
            true
        }
        Err(e) => {
            eprintln!("[rhino] video: vf reset loadfile failed: {e}");
            false
        }
    }
}
