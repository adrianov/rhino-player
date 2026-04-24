//! Rhino Player: GTK4 shell around libmpv. See `docs/`.

mod app;
mod mpv_embed;
mod theme;
mod time;

pub use app::run;
pub use time::format_time;
