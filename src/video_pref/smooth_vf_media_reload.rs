// Same-media `loadfile replace` for first Smooth-on while playing, or when `vf add` fails after a strip.

use std::cell::Cell;

thread_local! {
    /// Playing Smooth-on reload: stay paused until resume + apply, then unpause.
    static UNPAUSE_AFTER_SMOOTH_RELOAD: Cell<Option<crate::stall_log::Hold>> = const { Cell::new(None) };
}

/// Unpause if [reload_open_media_for_vf_reset] armed a playing-session hold.
pub(crate) fn maybe_unpause_after_smooth_reload(mpv: &libmpv2::Mpv) {
    let Some(hold) = UNPAUSE_AFTER_SMOOTH_RELOAD.take() else {
        return;
    };
    end_reload_hold(hold);
    if let Err(e) = mpv.set_property("pause", false) {
        eprintln!("[rhino] video: smooth-on unpause after reload failed: {e:?}");
        return;
    }
    eprintln!("[rhino] video: smooth-on unpause after reload");
}

/// Engine-held pause: blackout covers stay up across the reload (paired with [end_reload_hold]).
fn begin_reload_hold(mpv: &libmpv2::Mpv) -> crate::stall_log::Hold {
    crate::screen_blackout::begin_tech_hold();
    crate::stall_log::Hold::begin("smooth reload", mpv)
}

fn end_reload_hold(hold: crate::stall_log::Hold) {
    crate::screen_blackout::end_tech_hold();
    hold.end();
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
    let hold = resume_playing.then(|| begin_reload_hold(&b.mpv));
    let _ = b.mpv.set_property("pause", true);
    strip_vapoursynth_before_replace_media(b);
    let resume = if pos > 0.05 { Some(pos) } else { None };
    replace_media_at_pos(b, &path, pos, resume, hold)
}

/// `loadfile replace` at the playhead; on success arms the unpause hold and drains after load.
fn replace_media_at_pos(
    b: &MpvBundle,
    path: &std::path::Path,
    pos: f64,
    resume: Option<f64>,
    hold: Option<crate::stall_log::Hold>,
) -> bool {
    match b.load_file_path(path, false, false, false, resume) {
        Ok(()) => {
            eprintln!(
                "[rhino] video: loadfile replace for vapoursynth reattach path={} pos={pos:.2} resume_playing={}",
                path.display(),
                hold.is_some()
            );
            // Same media: do not re-run landscape fit / load-nudge (open + manual resize only).
            crate::app::suppress_window_fit_for_load(b.warm_file_gen());
            if hold.is_some() {
                UNPAUSE_AFTER_SMOOTH_RELOAD.set(hold);
            }
            // Idle only: callers hold `player` borrowed, so an immediate drain cannot `borrow_mut`.
            crate::app::transport_drain_after_loadfile_idle();
            true
        }
        Err(e) => {
            eprintln!("[rhino] video: vf reset loadfile failed: {e}");
            if let Some(hold) = hold {
                end_reload_hold(hold);
                let _ = b.mpv.set_property("pause", false);
            }
            false
        }
    }
}
