// Video prefs storage: VapourSynth ~60 fps vf switches + default ME pixel budget
// (docs/features/26-sixty-fps-motion.md). Split-out unit of the flat `db` module.

use rusqlite::{params, Connection, OptionalExtension};

/// Current key; bool `0`/`1`.
pub(super) const K_VIDEO_SMOOTH_60: &str = "video_smooth_60";
pub(super) const K_VIDEO_VS: &str = "video_vs_path";
pub(super) const K_VIDEO_MVTOOLS_LIB: &str = "video_mvtools_lib";
pub(super) const K_VIDEO_MANIPMV_LIB: &str = "video_manipmv_lib";
pub(super) const K_VIDEO_SMOOTH_MAX_AREA: &str = "video_smooth_max_area";

/// Width component of [`DEFAULT_SMOOTH_MAX_AREA`] (exact **1920×1080** ME raster).
pub const DEFAULT_SMOOTH_ME_WIDTH: u64 = 1920;
/// Height component of [`DEFAULT_SMOOTH_MAX_AREA`] (exact **1920×1080** ME raster).
pub const DEFAULT_SMOOTH_ME_HEIGHT: u64 = 1080;
/// Default ME/output pixel budget when the persistent store has no row (**exactly** **1920×1080** px²).
pub const DEFAULT_SMOOTH_MAX_AREA: u64 = DEFAULT_SMOOTH_ME_WIDTH * DEFAULT_SMOOTH_ME_HEIGHT;
/// Clamp loaded/saved smooth pixel budgets below this floor (**320×180**).
pub const MIN_SMOOTH_MAX_AREA: u64 = 320 * 180;

#[derive(Debug, Clone)]
pub struct VideoPrefs {
    /// When set: add mpv `vf=vapoursynth` with [vs_path] or bundled `.vpy` (+ presentation tuning — see feature 26 Notes).
    /// Default **off** until the user opts in; bundled script applies when `video_vs_path` is empty once enabled.
    pub smooth_60: bool,
    /// Absolute path to a `.vpy` for mpv’s `vapoursynth` filter, or empty for bundled script.
    pub vs_path: String,
    /// Cached absolute path to the **MVTools** plugin file (`libmvtools.so` on Linux,
    /// `libmvtools.dylib` on macOS) after a successful find; skipped on next call if still a file.
    pub mvtools_lib: String,
    /// Legacy SQLite field (`video_manipmv_lib`); unused by the bundled `.vpy`.
    pub manipmv_lib: String,
    /// Preferences default ME pixel budget for paths without their own **`media.smooth_me_budget_px2`**
    /// (exact **1920×1080** until the user changes it in **Preferences**). Adaptive overload/recovery updates the
    /// **`media`** row for the open file, not this field.
    pub smooth_max_area: u64,
}

impl Default for VideoPrefs {
    fn default() -> Self {
        Self {
            smooth_60: false,
            vs_path: String::new(),
            mvtools_lib: String::new(),
            manipmv_lib: String::new(),
            smooth_max_area: DEFAULT_SMOOTH_MAX_AREA,
        }
    }
}

fn setting_raw(c: &Connection, key: &str) -> Option<String> {
    c.query_row("SELECT v FROM settings WHERE k = ?1", params![key], |row| {
        row.get(0)
    })
    .optional()
    .unwrap_or(None)
}

fn put_setting(conn: &Connection, key: &str, val: &str) {
    let _ = conn.execute(
        "INSERT INTO settings (k, v) VALUES (?1, ?2)
         ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![key, val],
    );
}

/// Whether the one-shot migration [migrate_smooth_max_area_legacy_adaptive_pollution] already ran.
fn legacy_adaptive_reset_done(c: &Connection) -> bool {
    setting_raw(c, super::K_SMOOTH_MAX_AREA_LEGACY_ADAPTIVE_RESET_V1).as_deref() == Some("1")
}

/// Clamp a polluted low pref back to [`DEFAULT_SMOOTH_MAX_AREA`] (see the migration doc below).
fn clamp_polluted_pref(conn: &Connection) {
    let Some(s) = setting_raw(conn, K_VIDEO_SMOOTH_MAX_AREA) else {
        return;
    };
    let Ok(n) = s.trim().parse::<u64>() else {
        return;
    };
    if n.max(MIN_SMOOTH_MAX_AREA) >= DEFAULT_SMOOTH_MAX_AREA {
        return;
    }
    put_setting(
        conn,
        K_VIDEO_SMOOTH_MAX_AREA,
        &DEFAULT_SMOOTH_MAX_AREA.to_string(),
    );
}

/// Older builds wrote **adaptive overload** shrink into **`video_smooth_max_area`**, so new files inherited
/// another clip’s ME cap. Per-file values live on **`media.smooth_me_budget_px2`** now; prefs hold the default
/// for paths without a row. This migration runs **once** and clamps a polluted low pref back to [`DEFAULT_SMOOTH_MAX_AREA`].
pub(super) fn migrate_smooth_max_area_legacy_adaptive_pollution(conn: &Connection) {
    if legacy_adaptive_reset_done(conn) {
        return;
    }
    clamp_polluted_pref(conn);
    put_setting(conn, super::K_SMOOTH_MAX_AREA_LEGACY_ADAPTIVE_RESET_V1, "1");
}

/// Normalize legacy **`video_smooth_max_area`** values stored as round **2_000_000** px² (~HD) to exact **1920×1080**.
pub(super) fn migrate_legacy_smooth_max_area_round_mil(conn: &Connection) {
    let Some(v) = setting_raw(conn, K_VIDEO_SMOOTH_MAX_AREA) else {
        return;
    };
    if v.trim() != "2000000" {
        return;
    }
    let _ = conn.execute(
        "UPDATE settings SET v = ?2 WHERE k = ?1",
        params![K_VIDEO_SMOOTH_MAX_AREA, DEFAULT_SMOOTH_MAX_AREA.to_string(),],
    );
}
