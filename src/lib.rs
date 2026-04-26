//! Rhino Player: GTK4 shell around libmpv. See `docs/`.
//!
//! Copyright © Peter Adrianov, 2026. GPL-3.0-or-later.

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
mod media_probe;
mod mpv_embed;
mod paths;
mod recent_view;
pub mod sched;
mod sibling_advance;
mod theme;
mod time;
mod video_ext;
mod video_pref;

pub use app::{run, APP_ID};
pub use time::format_time;
