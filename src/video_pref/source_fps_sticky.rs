fn source_fps_from_env_var() -> Option<f64> {
    std::env::var(crate::paths::RHINO_SOURCE_FPS_VAR)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn db_source_fps_for_mpv(mpv: &libmpv2::Mpv) -> Option<f64> {
    crate::media_probe::local_file_from_mpv(mpv).and_then(|p| crate::db::media_source_fps(&p))
}

/// mpv often drops `container-fps` and ignores `estimated-vf-fps` while vapoursynth vf is active.
fn sticky_local_source_fps(gate: &FpsPickGateState) -> Option<f64> {
    gate.last_stable_fps
        .filter(|f| is_plausible_broadcast_fps(*f))
        .or_else(source_fps_from_env_var)
}

fn peek_sticky_local_source_fps(mpv: &libmpv2::Mpv) -> Option<f64> {
    db_source_fps_for_mpv(mpv).or_else(|| {
        FPS_PICK_GATE
            .lock()
            .ok()
            .and_then(|gate| sticky_local_source_fps(&gate))
    })
}

const FPS_READOUT_LO: f64 = 0.05;
const FPS_READOUT_HI: f64 = 960.0;

fn fps_readout_ok(f: f64) -> bool {
    f.is_finite() && f > FPS_READOUT_LO && f < FPS_READOUT_HI
}

fn round_fps_label(f: f64) -> String {
    format!("{}", f.round() as i64)
}

/// Source cadence for the Smooth tooltip (demux or sticky), `None` if no media.
pub fn source_fps_label(mpv: &libmpv2::Mpv) -> Option<String> {
    if !matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty()) {
        return None;
    }
    let fps = mpv
        .get_property::<f64>("container-fps")
        .ok()
        .filter(|&f| fps_readout_ok(f))
        .or_else(|| peek_sticky_local_source_fps(mpv).filter(|&f| fps_readout_ok(f)))?;
    Some(round_fps_label(fps))
}

/// Smooth toolbar badge: rounded **playing** FPS from mpv (`estimated-vf-fps`), never a selected-state target.
///
/// With **`vapoursynth`** loaded, skip estimates that still look like demux broadcast rates (stale
/// ~24 while FlowFPS is ramping) and show **—** until the filter-output estimate settles.
pub fn smooth_toolbar_fps_label(mpv: &libmpv2::Mpv) -> String {
    if !matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty()) {
        return "—".to_string();
    }
    let vs = vf_chain_has_vapoursynth(mpv);
    if let Ok(est) = mpv.get_property::<f64>("estimated-vf-fps") {
        if fps_readout_ok(est) && (!vs || !is_plausible_broadcast_fps(est)) {
            return round_fps_label(est);
        }
    }
    if vs {
        return "—".to_string();
    }
    let spd_raw = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let spd = if spd_raw.is_finite() && (0.01..=8.0).contains(&spd_raw) {
        spd_raw.max(FPS_READOUT_LO)
    } else {
        1.0
    };
    let nominal = mpv.get_property::<f64>("container-fps").unwrap_or(0.0);
    if fps_readout_ok(nominal) {
        return round_fps_label(nominal * spd);
    }
    if let Some(src) = peek_sticky_local_source_fps(mpv) {
        return round_fps_label(src * spd);
    }
    if let Some(src) = source_fps_from_env_var() {
        return round_fps_label(src * spd);
    }
    "—".to_string()
}
