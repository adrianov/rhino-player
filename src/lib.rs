//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright © 2026 Peter Adrianov. GPL-3.0-or-later.

mod app;
mod audio_tracks;
mod chapter_list;
mod continue_undo;
mod db;
mod dvd_ifo_parse;
mod dvd_entity;
mod dvd_vob_log;
mod dvd_vob_timeline;
mod fullscreen_timing;
mod glib_log_filter;
mod glib_source_drop;
mod header_menu_tracks;
mod history;
mod header_menu_scroll;
mod human_media_title;
mod icons;
mod idle_inhibit;
mod jpeg_texture;
#[cfg(target_os = "macos")]
mod macos_fs_debug;
#[cfg(target_os = "macos")]
mod macos_fs_exit;
#[cfg(target_os = "macos")]
mod macos_bottom_bar;
#[cfg(target_os = "macos")]
mod macos_header_menu;
#[cfg(target_os = "macos")]
mod macos_header_menu_debug;
#[cfg(target_os = "macos")]
mod macos_header_menu_overlay;
#[cfg(target_os = "macos")]
mod macos_open_video;
#[cfg(target_os = "macos")]
mod macos_window;
mod media_probe;
mod mpris;
mod mpv_embed;
mod paths;
mod playback_entity;
mod playback_speed;
mod recent_view;
mod screen_blackout;
pub mod sched;
mod seek_bar_preview;
mod sibling_advance;
mod shell_debug_log;
mod sub_prefs;
mod sub_track_abbr;
mod sub_tracks;
mod theme;
mod theme_cursor;
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
