//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright © 2026 Peter Adrianov. GPL-3.0-or-later.

mod app;
mod audio_tracks;
mod black_bars;
mod chapter_list;
mod continue_undo;
mod db;
mod diagnostics;
mod dvd_entity;
mod dvd_ifo_parse;
mod dvd_vob_log;
mod dvd_vob_mpv_probe;
mod dvd_vob_timeline;
mod fullscreen_timing;
mod glib_log_filter;
mod glib_source_drop;
mod header_menu_scroll;
mod header_menu_tracks;
mod history;
mod human_media_title;
mod icons;
mod idle_inhibit;
mod incomplete_download_eof;
#[cfg(target_os = "macos")]
mod macos_bottom_bar;
#[cfg(target_os = "macos")]
mod macos_drag_drop;
#[cfg(target_os = "macos")]
mod macos_fs_debug;
#[cfg(target_os = "macos")]
mod macos_fs_exit;
#[cfg(target_os = "macos")]
mod macos_header_menu;
#[cfg(target_os = "macos")]
mod macos_header_menu_debug;
#[cfg(target_os = "macos")]
mod macos_header_menu_overlay;
#[cfg(target_os = "macos")]
mod macos_open_video;
#[cfg(target_os = "macos")]
mod macos_shell_compositing;
#[cfg(target_os = "macos")]
mod macos_window;
mod media_open_fail;
mod media_probe;
mod mpris;
mod mpv_embed;
mod paths;
mod playback_entity;
mod playback_speed;
mod preview_debug;
mod recent_view;
mod reveal_file;
pub mod sched;
mod screen_blackout;
mod seek_bar_preview;
mod shell_debug_log;
mod sibling_advance;
mod sub_prefs;
mod sub_track_abbr;
mod sub_tracks;
mod theme;
mod thumb_texture;
mod time;
mod track_label_match;
mod track_menu_label;
#[cfg(target_os = "macos")]
mod trash_macos;
mod trash_xdg;
mod user_action_log;
mod video_ext;
mod video_fill;
mod video_pref;
mod window_present;

pub use app::{run, APP_ID};
pub use diagnostics::{cli_diagnostics_exit, cli_version_exit};
pub use time::format_time;

#[cfg(target_os = "macos")]
pub use paths::{macos_prime_homebrew_runtime_env, macos_reexec_for_vapoursynth_dyld_if_needed};
