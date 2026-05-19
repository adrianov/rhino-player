//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright © 2026 Peter Adrianov. GPL-3.0-or-later.

mod app;
mod audio_tracks;
mod chapter_list;
mod continue_undo;
mod db;
mod fullscreen_timing;
mod glib_log_filter;
mod history;
mod human_media_title;
mod icons;
mod idle_inhibit;
mod jpeg_texture;
#[cfg(target_os = "macos")]
mod macos_window;
mod media_probe;
mod mpris;
mod mpv_embed;
mod paths;
mod playback_speed;
mod recent_view;
pub mod sched;
mod seek_bar_preview;
mod sibling_advance;
mod sub_prefs;
mod sub_track_abbr;
mod sub_tracks;
mod theme;
mod window_present;
mod time;
mod track_label_match;
#[cfg(target_os = "macos")]
mod trash_macos;
mod trash_xdg;
mod video_ext;
mod video_pref;

pub use app::{run, APP_ID};
pub use time::format_time;
