//! MPRIS2 on the Freedesktop session bus (Linux). Other targets compile empty stubs.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use libmpv2::Mpv;

use crate::mpv_embed::MpvBundle;

#[cfg(target_os = "linux")]
pub(crate) struct MpvSeekAbs(pub(crate) Rc<dyn Fn(&str)>);

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub(crate) struct MpvSeekAbs;

/// Snapshot for D-Bus property sync from the GTK main thread transport path.
#[derive(Clone, Debug)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) struct MprisShot {
    pub(crate) paused: bool,
    pub(crate) pos_sec: f64,
    pub(crate) dur_sec: f64,
    pub(crate) path_open: bool,
    pub(crate) stopped: bool,
    pub(crate) title: Option<String>,
    pub(crate) track_path: Option<PathBuf>,
    pub(crate) can_prev: bool,
    pub(crate) can_next: bool,
}

/// Wire-up for [`start_linux`].
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) struct MprisStartArgs {
    pub(crate) app: adw::Application,
    pub(crate) win: adw::ApplicationWindow,
    pub(crate) mpv_bundle: Rc<RefCell<Option<MpvBundle>>>,
    pub(crate) seek_abs: MpvSeekAbs,
    pub(crate) toggle_play_pause: Rc<dyn Fn()>,
    pub(crate) pause_only: Rc<dyn Fn()>,
    pub(crate) unpause_only: Rc<dyn Fn()>,
    pub(crate) stop: Rc<dyn Fn()>,
    pub(crate) prev: Rc<dyn Fn()>,
    pub(crate) next: Rc<dyn Fn()>,
}

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub(crate) use linux::{enqueue_snapshot, start_linux};

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub(crate) fn start_linux(_: MprisStartArgs) {}

#[cfg(not(target_os = "linux"))]
pub(crate) fn enqueue_snapshot(_: MprisShot) {}

pub(crate) fn mpv_has_open_path(mpv: &Mpv) -> bool {
    matches!(mpv.get_property::<String>("path"), Ok(s) if !s.trim().is_empty())
}
