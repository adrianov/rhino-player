// Raw mpv transport property sampling and duration clamping helpers.
/// Raw mpv transport properties for the live bundle.
fn read_mpv_transport_props(b: &MpvBundle) -> (bool, bool, bool, f64, f64) {
    (
        b.mpv.get_property::<bool>("pause").unwrap_or(false),
        b.mpv.get_property::<bool>("core-idle").unwrap_or(false),
        b.mpv.get_property::<bool>("eof-reached").unwrap_or(false),
        clamp_mpv_sec(b.mpv.get_property::<f64>("time-pos").unwrap_or(0.0)),
        clamp_mpv_sec(b.mpv.get_property::<f64>("duration").unwrap_or(0.0)),
    )
}
fn clamp_mpv_sec(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

/// Container `duration` can exceed the decoded tail; clamp for transport UI when stalled near end.
fn duration_clamp_stalled_playout(
    dur: f64,
    pos: f64,
    core_idle: bool,
    eof_reached: bool,
    played_into_tail: bool,
) -> f64 {
    if dur <= 0.0 || pos <= 0.0 {
        return dur;
    }
    let gap = dur - pos;
    if gap > 0.0
        && gap <= crate::media_probe::NEAR_END_SEC
        && (eof_reached || (core_idle && played_into_tail))
    {
        pos
    } else {
        dur
    }
}
