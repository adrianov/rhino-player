// Smooth cadence gate: disc fps lock, interleaved display-resample vs MVTools.
include!("mvtools_fps_pick_state.rs");

/// Blu-ray folder / `bd://` titles: cadence is trustworthy enough for MVTools when `container-fps`,
/// a disc lock, or a persisted shell rate exists (does not consult [FpsPickGateState::interleaved_smooth]).
fn bluray_cadence_known(
    mpv: &libmpv2::Mpv,
    shell_media: Option<&std::path::Path>,
    gate: &FpsPickGateState,
) -> bool {
    let Some(shell) = shell_media.filter(|p| crate::video_ext::is_bluray_disc_path(p)) else {
        return false;
    };
    if mpv
        .get_property::<f64>("container-fps")
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .is_some_and(is_plausible_broadcast_fps)
    {
        return true;
    }
    if gate.locked_disc_fps.is_some_and(is_plausible_broadcast_fps) {
        return true;
    }
    crate::db::media_source_fps(shell).is_some_and(is_plausible_broadcast_fps)
}

static FPS_PICK_GATE: Mutex<FpsPickGateState> = Mutex::new(FpsPickGateState {
    last_path: None,
    ignore_est_left: 0,
    locked_disc_fps: None,
    interleaved_smooth: false,
    stable_streak: 0,
    last_stable_fps: None,
});

/// After a **seek**, mpv cadence readings fluctuate on interleaved discs — stay on display-resample until stable.
fn mark_smooth_cadence_unstable_after_seek() {
    let mut g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    g.interleaved_smooth = true;
    g.stable_streak = 0;
    g.last_stable_fps = None;
    g.locked_disc_fps = None;
}

fn mpv_current_path(mpv: &libmpv2::Mpv) -> Option<String> {
    mpv.get_property::<String>("path")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn smooth_cadence_unstable_target(mpv: &libmpv2::Mpv) -> bool {
    let path = mpv_current_path(mpv);
    path.as_deref().is_some_and(mpv_path_is_disc)
        || path_str_is_dvd_vob(path.as_deref())
        || crate::media_probe::local_file_from_mpv(mpv)
            .is_some_and(|p| crate::video_ext::is_dvd_vob_path(&p))
}

/// Seek on optical-disc media: prefer display-resample until cadence stabilizes.
pub(crate) fn mark_smooth_cadence_unstable_after_seek_if_disc(mpv: &libmpv2::Mpv) {
    if smooth_cadence_unstable_target(mpv) {
        mark_smooth_cadence_unstable_after_seek();
    }
}

/// True when Smooth 60 should use mpv **display-resample** only (no VapourSynth / cadence rebuild).
pub(crate) fn smooth_prefers_display_resample(
    mpv: &libmpv2::Mpv,
    shell_disc: Option<&std::path::Path>,
    shell_media: Option<&std::path::Path>,
) -> bool {
    let path_now = mpv_current_path(mpv);
    let g = FPS_PICK_GATE.lock().unwrap_or_else(|e| e.into_inner());
    if shell_path_is_dvd_vob(shell_media) || path_str_is_dvd_vob(path_now.as_deref()) {
        return g.interleaved_smooth;
    }
    prefers_display_resample_on_disc(mpv, shell_disc, shell_media, path_now.as_deref(), &g)
}

/// Non-DVD-VOB media: Blu-ray cadence short-circuits; interleaved discs settle on reads.
fn prefers_display_resample_on_disc(
    mpv: &libmpv2::Mpv,
    shell_disc: Option<&std::path::Path>,
    shell_media: Option<&std::path::Path>,
    path_now: Option<&str>,
    g: &FpsPickGateState,
) -> bool {
    let on_bd_protocol = path_now.is_some_and(mpv_path_is_disc);
    let disc = on_bd_protocol || shell_disc.is_some_and(crate::video_ext::is_optical_disc_path);
    // Folder-open Blu-ray (`me_budget_shell_path`): skip the 3-read settle once cadence is known.
    if !on_bd_protocol && bluray_cadence_known(mpv, shell_media, g) {
        return false;
    }
    if g.interleaved_smooth {
        return true;
    }
    if bluray_cadence_known(mpv, shell_media, g) {
        return false;
    }
    disc && g.stable_streak < CADENCE_STABLE_READS
}
