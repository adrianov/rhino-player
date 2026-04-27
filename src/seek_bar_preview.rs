//! Seek bar hover: a **second** [libmpv] with [vo=libmpv] in a small [`gtk::GLArea`]
//! (same OpenGL path as the main [crate::mpv_embed::MpvBundle] — not `screenshot-raw`).

include!("seek_bar_preview/state_and_vo_pump.rs");
include!("seek_bar_preview/connect_popover_wiring.rs");
