//! Rhino Player: GTK4 shell around libmpv. See `docs/`.

mod app;
mod db;
mod history;
mod media_probe;
mod mpv_embed;
mod paths;
mod recent_view;
mod theme;
mod time;

pub use app::run;
pub use time::format_time;
