//! Black out non-viewer displays while playing (macOS). See `docs/features/17-window-behavior.md`.

use crate::mpv_embed::MpvBundle;
use glib::prelude::ObjectExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, WidgetExt};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const TOOLTIP: &str = "Black out other displays while playing";
const ICON: &str = "video-display-symbolic";

thread_local! {
    /// Depth of engine-held pauses (smooth `vf` swap, seek burst, chapter scrub) — not user pause.
    static TECH_HOLD: Cell<u32> = const { Cell::new(0) };
}

/// Keep blackout up across an engine-held pause.
pub fn begin_tech_hold() {
    TECH_HOLD.with(|d| d.set(d.get().saturating_add(1)));
}

/// Pair with [begin_tech_hold] when that hold ends.
pub fn end_tech_hold() {
    TECH_HOLD.with(|d| d.set(d.get().saturating_sub(1)));
}

fn tech_hold_active() -> bool {
    TECH_HOLD.with(|d| d.get() > 0)
}

/// Shared handle for toolbar wiring and transport-driven refresh.
pub struct BlackoutSync {
    blackout: Rc<RefCell<ScreenBlackout>>,
    win: adw::ApplicationWindow,
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::Box,
    btn: gtk::Button,
    /// Needs a pass; cleared at the start of [Self::flush].
    dirty: Cell<bool>,
    /// A GLib idle is already queued to run [Self::flush].
    scheduled: Cell<bool>,
}

impl BlackoutSync {
    /// Coalesce on a GLib idle (GTK-safe). AppKit show/hide is queued to libdispatch from there.
    pub fn sync(self: &Rc<Self>) {
        self.dirty.set(true);
        if self.scheduled.replace(true) {
            return;
        }
        let this = Rc::clone(self);
        let _ = glib::idle_add_local_once(move || {
            this.scheduled.set(false);
            this.flush();
            if this.dirty.get() {
                this.sync();
            }
        });
    }

    fn flush(&self) {
        self.dirty.set(false);
        sync_btn_visible(&self.btn);
        let recent_visible = self.recent.is_visible();
        #[cfg(target_os = "macos")]
        sync_macos(
            &self.blackout,
            &self.win,
            &self.player,
            recent_visible,
        );
        #[cfg(not(target_os = "macos"))]
        let _ = recent_visible;
    }
}

/// True when the platform reports at least two connected displays.
pub fn multi_screen() -> bool {
    #[cfg(target_os = "macos")]
    {
        screen_count_macos() >= 2
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Overlay windows covering every display except the viewer's.
pub struct ScreenBlackout {
    enabled: bool,
    #[cfg(target_os = "macos")]
    windows: Vec<objc2::rc::Retained<objc2_app_kit::NSWindow>>,
    #[cfg(target_os = "macos")]
    video_screen_ptr: Option<*const objc2_app_kit::NSScreen>,
    #[cfg(target_os = "macos")]
    last_screen_count: usize,
    /// Rebuild queued on libdispatch; skip duplicate plans until it lands or is cleared.
    #[cfg(target_os = "macos")]
    cover_pending: bool,
}

impl ScreenBlackout {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            #[cfg(target_os = "macos")]
            windows: Vec::new(),
            #[cfg(target_os = "macos")]
            video_screen_ptr: None,
            #[cfg(target_os = "macos")]
            last_screen_count: 0,
            #[cfg(target_os = "macos")]
            cover_pending: false,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        crate::db::save_black_out_screens(on);
    }
}

include!("screen_blackout_toolbar.rs");

#[cfg(target_os = "macos")]
include!("screen_blackout_macos.rs");
#[cfg(target_os = "macos")]
include!("screen_blackout_observe_macos.rs");
