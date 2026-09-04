// App settings (key-value): master volume/mute, UI toggles, last audio track label.
// Split-out unit of the flat `db` module; public paths (`db::load_audio`, …) are re-exported.

use rusqlite::{params, Connection, OptionalExtension};

use super::with_conn;

const K_VOL: &str = "master_volume";
const K_MUTE: &str = "master_mute";
const K_AUDIO_TRACK_NAME: &str = "audio_track_name";

fn raw_setting(c: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    c.query_row("SELECT v FROM settings WHERE k = ?1", params![key], |row| {
        let s: String = row.get(0)?;
        Ok(s)
    })
    .optional()
}

fn stored_bool(o: Option<String>, default: bool) -> bool {
    o.map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn parse_stored_volume(o: Option<String>) -> f64 {
    o.and_then(|s| s.parse::<f64>().ok())
        .filter(|x: &f64| x.is_finite())
        .map(|x| x.clamp(0.0, 200.0))
        .unwrap_or(100.0)
}

fn put_setting(key: &str, val: &str) {
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![key, val],
        )?;
        Ok(())
    });
}

/// Last saved `libmpv` `volume` (0…`volume-max`, typically 0…100) and `mute` from the previous run.
pub fn load_audio() -> (f64, bool) {
    (
        with_conn(|c| Ok(parse_stored_volume(raw_setting(c, K_VOL)?))).unwrap_or(100.0),
        with_conn(|c| Ok(stored_bool(raw_setting(c, K_MUTE)?, false))).unwrap_or(false),
    )
}

pub fn save_audio(volume: f64, muted: bool) {
    if !volume.is_finite() {
        return;
    }
    let v = volume.clamp(0.0, 200.0);
    let _ = with_conn(|c| {
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_VOL, format!("{v:.4}")],
        )?;
        c.execute(
            "INSERT INTO settings (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![K_MUTE, if muted { "1" } else { "0" }],
        )?;
        Ok(())
    });
}

/// Persist for the next app launch. Safe to call from the quit path before [commit_quit].
const K_SEEK_BAR_PREVIEW: &str = "seek_bar_preview";

/// [docs/features/18-thumbnail-preview.md] — `true` by default.
pub fn load_seek_bar_preview() -> bool {
    with_conn(|c| Ok(stored_bool(raw_setting(c, K_SEEK_BAR_PREVIEW)?, true))).unwrap_or(true)
}

pub fn save_seek_bar_preview(on: bool) {
    put_setting(K_SEEK_BAR_PREVIEW, if on { "1" } else { "0" });
}

const K_BLACK_OUT_SCREENS: &str = "black_out_screens";

/// [docs/features/17-window-behavior.md] — multi-monitor blackout; default off.
pub fn load_black_out_screens() -> bool {
    with_conn(|c| Ok(stored_bool(raw_setting(c, K_BLACK_OUT_SCREENS)?, false))).unwrap_or(false)
}

pub fn save_black_out_screens(on: bool) {
    put_setting(K_BLACK_OUT_SCREENS, if on { "1" } else { "0" });
}

pub fn load_audio_track_name() -> Option<String> {
    super::get_setting_str(K_AUDIO_TRACK_NAME)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn save_audio_track_name(name: &str) {
    let s = name.trim();
    if s.is_empty() {
        return;
    }
    put_setting(K_AUDIO_TRACK_NAME, s);
}
