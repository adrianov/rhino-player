fn finite_playhead(t: f64) -> Option<f64> {
    (t.is_finite() && t >= 0.0).then_some(t)
}

/// Post-**`vf`** seek second: stashed resume wins, then **`playback-time`**, then **`time-pos`**.
pub(crate) fn vf_resync_sec_from_sources(
    pending_resume: Option<f64>,
    playback_time: Option<f64>,
    time_pos: Option<f64>,
) -> Option<f64> {
    pending_resume
        .filter(|t| t.is_finite() && *t >= 0.0)
        .or(playback_time.and_then(finite_playhead))
        .or(time_pos.and_then(finite_playhead))
}

/// Seconds for post-**`vf`** exact seeks: stashed resume (if any), then **`playback-time`**, then **`time-pos`**.
pub(crate) fn vf_resync_playhead_sec(
    mpv: &Mpv,
    bundle: Option<&crate::mpv_embed::MpvBundle>,
) -> Option<f64> {
    let pending = bundle.and_then(|b| b.stashed_resume_sec());
    let playback_time = mpv
        .get_property::<f64>("playback-time")
        .ok()
        .and_then(finite_playhead);
    let time_pos = mpv
        .get_property::<f64>("time-pos")
        .ok()
        .and_then(finite_playhead);
    vf_resync_sec_from_sources(pending, playback_time, time_pos)
}
