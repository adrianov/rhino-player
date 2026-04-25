//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright (c) Peter Adrianov, 2026. GPL-3.0-or-later.

mod app;
mod audio_tracks;
mod sub_prefs;
mod sub_tracks;
mod icons;
mod db;
mod history;
mod media_probe;
mod mpv_embed;
mod paths;
mod recent_view;
mod sibling_advance;
mod theme;
mod time;
mod video_ext;
mod video_pref;

pub use app::{run, APP_ID};
pub use time::format_time;
