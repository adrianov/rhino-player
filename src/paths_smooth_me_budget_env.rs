// Bundled ME px²: `RHINO_SMOOTH_MAX_AREA` before `vf add` (read by bundled `.vpy` via libc `getenv`).

/// Env key: decoded **width × height** cap (px²) for bundled MVTools **ME** / **FlowFPS** raster.
pub const RHINO_SMOOTH_MAX_AREA_VAR: &str = "RHINO_SMOOTH_MAX_AREA";

/// Publishes **`cap_px`** as decimal ASCII under [RHINO_SMOOTH_MAX_AREA_VAR] before **`vf add`**.
pub fn publish_smooth_me_budget_env(cap_px: u64) {
    std::env::set_var(RHINO_SMOOTH_MAX_AREA_VAR, format!("{cap_px}"));
}

/// **`true`** when [RHINO_SMOOTH_MAX_AREA_VAR] parses to **`want_px`**.
#[must_use]
pub fn smooth_max_area_env_matches(want_px: u64) -> bool {
    std::env::var(RHINO_SMOOTH_MAX_AREA_VAR)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        == Some(want_px)
}
