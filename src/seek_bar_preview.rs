//! Seek bar hover: a **second** [libmpv] with [vo=libmpv] in a small [`gtk::GLArea`]
//! (same OpenGL path as the main [crate::mpv_embed::MpvBundle] — not `screenshot-raw`).
//! The preview is a plain overlay child of the window's `GtkOverlay`, not a `GtkPopover`,
//! so it never creates a new compositor surface and never causes a full-window repaint.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;

use crate::format_time;
use crate::media_probe::local_file_from_mpv;
use crate::mpv_embed::{set_preview_tracks, MpvBundle, MpvPreviewGl};

include!("seek_bar_preview/state_and_vo_pump.rs");
include!("seek_bar_preview/connect_popover_wiring.rs");
