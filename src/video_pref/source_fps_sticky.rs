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

/// Header Smooth toolbar: rounded **playing** frame rate (`estimated-vf-fps` when known, else source×speed).
pub fn smooth_toolbar_fps_label(mpv: &libmpv2::Mpv) -> String {
    const LO: f64 = 0.05;
    const HI: f64 = 960.0;
    if !matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty()) {
        return "—".to_string();
    }
    let spd_raw = mpv.get_property::<f64>("speed").unwrap_or(1.0);
    let spd = if spd_raw.is_finite() && (0.01..=8.0).contains(&spd_raw) {
        spd_raw.max(LO)
    } else {
        1.0
    };
    if let Ok(est) = mpv.get_property::<f64>("estimated-vf-fps") {
        if est.is_finite() && est > LO && est < HI {
            return format!("{}", est.round() as i64);
        }
    }
    if vf_chain_has_vapoursynth(mpv) {
        return "60".to_string();
    }
    let nominal = mpv.get_property::<f64>("container-fps").unwrap_or(0.0);
    if nominal.is_finite() && nominal > LO && nominal < HI {
        return format!("{}", (nominal * spd).round() as i64);
    }
    if let Some(src) = peek_sticky_local_source_fps(mpv) {
        return format!("{}", (src * spd).round() as i64);
    }
    if let Some(src) = source_fps_from_env_var() {
        return format!("{}", (src * spd).round() as i64);
    }
    "—".to_string()
}
