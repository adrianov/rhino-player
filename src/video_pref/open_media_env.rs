/// Set bundled ME cap from prefs + `media` **before** mpv **`loadfile`** (or warm reopen).
/// When **`clear_source_fps`**, drop **`RHINO_SOURCE_FPS`** so cadence is not inherited from a
/// **different** file. Warm preload (**same** clip already decoded) skips the clear so
/// [apply_mpv_video] does not treat **`None`→`Some(same_hz)`** as a cadence change and run **`vf clr`/add**.
pub fn publish_smooth_env_before_load(
    path: &std::path::Path,
    v: &crate::db::VideoPrefs,
    clear_source_fps: bool,
) {
    let global = v.smooth_max_area.max(crate::db::MIN_SMOOTH_MAX_AREA);
    let cap = crate::db::resolve_media_smooth_me_budget(Some(path), global);
    crate::paths::publish_smooth_me_budget_env(cap);
    if clear_source_fps {
        std::env::remove_var(crate::paths::RHINO_SOURCE_FPS_VAR);
    }
}
