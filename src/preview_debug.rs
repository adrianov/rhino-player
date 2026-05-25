//! Seek-bar preview diagnostics.

use libmpv2::Mpv;
use std::sync::OnceLock;

pub(crate) fn verbose() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RHINO_PREVIEW_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

/// High-signal lifecycle (always on plain `cargo run`).
pub(crate) fn info(msg: impl std::fmt::Display) {
    eprintln!("[rhino] preview: {msg}");
}

/// Per-tick / per-frame detail (`RHINO_PREVIEW_DEBUG=1`).
pub(crate) fn log(msg: impl std::fmt::Display) {
    if verbose() {
        eprintln!("[rhino] preview: {msg}");
    }
}

/// Failures and aborts — always printed.
pub(crate) fn warn(msg: impl std::fmt::Display) {
    eprintln!("[rhino] preview: {msg}");
}

pub(crate) fn mpv_line(mpv: &Mpv) -> String {
    let path = mpv.get_property::<String>("path").unwrap_or_default();
    let vo = mpv
        .get_property::<bool>("vo-configured")
        .map(|b| b.to_string())
        .unwrap_or_else(|_| "?".into());
    let dur = mpv
        .get_property::<f64>("duration")
        .ok()
        .filter(|d| d.is_finite())
        .map(|d| format!("{d:.2}"))
        .unwrap_or_else(|| "?".into());
    let dw = mpv.get_property::<i64>("dwidth").unwrap_or(0);
    let dh = mpv.get_property::<i64>("dheight").unwrap_or(0);
    format!("path={path} vo={vo} dur={dur} {dw}x{dh}")
}

/// True when the open target changed (not the same file or same playback entity).
#[must_use]
pub(crate) fn open_target_entity_changed(a: &std::path::Path, b: &std::path::Path) -> bool {
    let ra = crate::video_ext::resolve_open_media_path(a);
    let rb = crate::video_ext::resolve_open_media_path(b);
    if crate::video_ext::paths_same_file(&ra, &rb) {
        return false;
    }
    crate::playback_entity::PlaybackEntity::resolve(&ra).db_path()
        != crate::playback_entity::PlaybackEntity::resolve(&rb).db_path()
}
