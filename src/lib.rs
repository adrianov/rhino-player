//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright (C) 2026 Peter Adrianov. GPL-3.0-or-later.

mod app;
mod audio_tracks;
mod continue_undo;
mod db;
mod history;
mod icons;
mod idle_inhibit;
mod jpeg_texture;
mod media_probe;
mod mpv_embed;
mod paths;
mod playback_speed;
mod recent_view;
pub mod sched;
mod seek_bar_preview;
mod sibling_advance;
mod sub_prefs;
mod sub_tracks;
mod theme;
mod time;
mod trash_xdg;
mod video_ext;
mod video_pref;

pub use app::{run, APP_ID};
pub use time::format_time;
