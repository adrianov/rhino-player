// Same-media `loadfile replace` for first Smooth-on while playing, or when `vf add` fails after a strip.

use std::cell::Cell;

thread_local! {
    /// Playing Smooth-on reload: stay paused until resume + apply, then unpause.
    static UNPAUSE_AFTER_SMOOTH_RELOAD: Cell<bool> = const { Cell::new(false) };
}

/// Unpause if [reload_open_media_for_vf_reset] armed a playing-session hold.
pub(crate) fn maybe_unpause_after_smooth_reload(mpv: &libmpv2::Mpv) {
    if !UNPAUSE_AFTER_SMOOTH_RELOAD.with(|c| c.replace(false)) {
        return;
    }
    if let Err(e) = mpv.set_property("pause", false) {
        eprintln!("[rhino] video: smooth-on unpause after reload failed: {e:?}");
        return;
    }
    eprintln!("[rhino] video: smooth-on unpause after reload");
}

/// Reload the open file at the current playhead so mpv can attach vapoursynth again.
///
/// Holds **`pause`** through **`loadfile replace`** and resume so playback does not start at t=0
/// before the playhead is restored. When `resume_playing`, arms [maybe_unpause_after_smooth_reload].
/// No **`stop`** beforehand — it can abort the following load and leave resume pending while paused.
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
    let _ = b.mpv.set_property("pause", true);
    strip_vapoursynth_before_replace_media(b);
    let resume = if pos > 0.05 { Some(pos) } else { None };
    match b.load_file_path(&path, false, false, false, resume) {
        Ok(()) => {
            eprintln!(
                "[rhino] video: loadfile replace for vapoursynth reattach path={} pos={pos:.2} resume_playing={resume_playing}",
                path.display()
            );
            if resume_playing {
                UNPAUSE_AFTER_SMOOTH_RELOAD.with(|c| c.set(true));
            }
            // Idle only: callers hold `player` borrowed, so an immediate drain cannot `borrow_mut`.
            crate::app::transport_drain_after_loadfile_idle();
            true
        }
        Err(e) => {
            eprintln!("[rhino] video: vf reset loadfile failed: {e}");
            if resume_playing {
                let _ = b.mpv.set_property("pause", false);
            }
            false
        }
    }
}
