//! Rhino Player: GTK4 shell around libmpv. See `docs/`.

mod app;
mod audio_tracks;
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

pub use app::{run, APP_ID};
pub use time::format_time;
