//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright (C) 2026 Peter Adrianov. GPL-3.0-or-later.

mod app;
mod continue_undo;
mod trash_xdg;
mod audio_tracks;
mod sub_prefs;
mod sub_tracks;
mod icons;
mod db;
mod history;
mod idle_inhibit;
mod jpeg_texture;
mod media_probe;
mod mpv_embed;
mod paths;
mod playback_speed;
mod recent_view;
mod seek_bar_preview;
pub mod sched;
mod sibling_advance;
mod theme;
mod time;
mod video_ext;
mod video_pref;

pub use app::{run, APP_ID};
pub use time::format_time;
