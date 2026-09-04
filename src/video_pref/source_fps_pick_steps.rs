// Step helpers for [source_fps_from_mpv] (included by `mvtools_video_log_env.rs`):
// raw mpv reads, estimate gating, DVD VOB override, sticky fallback, persistence.

/// Raw mpv reads: container fps, estimated-vf fps (finite & positive), trimmed non-empty path.
fn read_source_fps_props(mpv: &libmpv2::Mpv) -> (Option<f64>, Option<f64>, Option<String>) {
    (
        finite_positive_prop(mpv, "container-fps"),
        finite_positive_prop(mpv, "estimated-vf-fps"),
        non_empty_trimmed_prop(mpv),
    )
}

/// mpv float property that is finite and positive.
fn finite_positive_prop(mpv: &libmpv2::Mpv, key: &str) -> Option<f64> {
    mpv.get_property::<f64>(key)
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// mpv `path`, trimmed; empty while idle / before the first `loadfile`.
fn non_empty_trimmed_prop(mpv: &libmpv2::Mpv) -> Option<String> {
    mpv.get_property::<String>("path")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Container/estimate pick, disc stabilization, DVD VOB override, then the local sticky fallback.
/// Returns `(picked, mpv_pick, dvd_pick)` — `mpv_pick` is the pre-override read (persisted in
/// preference to a DVD override), `dvd_pick` the VOB override if one applied.
fn pick_cadence_candidate(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    cfps: Option<f64>,
    est_raw: Option<f64>,
    path_now: Option<String>,
    shell_media: Option<&std::path::Path>,
    gate: &mut FpsPickGateState,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let picked = source_fps_from_container_and_estimated(
        cfps,
        gate_est_for_source_pick(path_now.clone(), est_raw, mpv, shell_media, gate),
    );
    let mpv_pick = picked;
    let mut picked = stabilize_disc_source_fps(path_now.as_deref(), shell_media, picked, gate);
    let dvd_pick = dvd_vob_gate_pick(mpv, bundle, gate);
    if dvd_pick.is_some() {
        picked = dvd_pick;
    }
    if picked.is_none() && !path_now.as_deref().is_some_and(mpv_path_is_disc) && dvd_pick.is_none()
    {
        picked = local_sticky_source_fps_fallback(shell_media, gate);
    }
    (picked, mpv_pick, dvd_pick)
}

/// Demux often reports ~24 Hz before film cadence stabilizes; `mask_est_for_path_change` drops
/// briefly-stale estimates, disc paths ignore them entirely, and vf-output rates (~60 Hz /
/// display-resample) are not source cadence.
fn gate_est_for_source_pick(
    path_now: Option<String>,
    est: Option<f64>,
    mpv: &libmpv2::Mpv,
    shell_media: Option<&std::path::Path>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    let est = mask_est_for_path_change_with_state(path_now.clone(), est, gate, shell_media);
    let est = if ignore_est_for_source_pick(path_now.as_deref(), mpv, shell_media) {
        None
    } else {
        est
    };
    est.filter(|e| is_plausible_broadcast_fps(*e))
}

/// DVD VOB override: broadcast fps from the decoded size (25 Hz fallback); pins the cadence gate.
/// Returns the override fps, or `None` when the open media is not a DVD VOB.
fn dvd_vob_gate_pick(
    mpv: &libmpv2::Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
    gate: &mut FpsPickGateState,
) -> Option<f64> {
    if !media_is_dvd_vob(mpv, bundle) {
        return None;
    }
    let fps = crate::video_ext::dvd_vob_broadcast_fps(crate::video_pref::decode_wh_from_mpv(mpv))
        .or(Some(25.0));
    gate.interleaved_smooth = false;
    gate.stable_streak = CADENCE_STABLE_READS;
    gate.last_stable_fps = fps;
    fps
}

/// No cadence read at all for a local file: persisted per-file fps, then sticky local sources.
fn local_sticky_source_fps_fallback(
    shell_media: Option<&std::path::Path>,
    gate: &FpsPickGateState,
) -> Option<f64> {
    shell_media
        .and_then(crate::db::media_source_fps)
        .or_else(|| sticky_local_source_fps(gate))
}

/// Persist the winning pick for the open local file (mpv read preferred, DVD override second).
fn persist_picked_source_fps(
    shell_media: Option<&std::path::Path>,
    mpv_pick: Option<f64>,
    dvd_pick: Option<f64>,
) {
    let Some(path) = shell_media else {
        return;
    };
    if let Some(fps) = mpv_pick.and_then(crate::db::snap_broadcast_fps_hz) {
        crate::db::media_save_source_fps(path, fps);
    } else if let Some(fps) = dvd_pick.and_then(crate::db::snap_broadcast_fps_hz) {
        crate::db::media_save_source_fps(path, fps);
    }
}

/// Cadence-gate update, Hz snapping, and persistence of the winning pick.
fn finalize_source_fps_pick(
    gate: &mut FpsPickGateState,
    picked: Option<f64>,
    mpv_pick: Option<f64>,
    dvd_pick: Option<f64>,
    path_now: Option<&str>,
    shell_media: Option<&std::path::Path>,
) -> Option<f64> {
    let picked = update_interleaved_cadence_gate(path_now, shell_media, picked, gate);
    let picked = picked.and_then(crate::db::snap_broadcast_fps_hz);
    persist_picked_source_fps(shell_media, mpv_pick, dvd_pick);
    picked
}
