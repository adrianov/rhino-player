//! Black out non-viewer displays while playing (macOS). See `docs/features/17-window-behavior.md`.

use crate::mpv_embed::MpvBundle;
use glib::prelude::ObjectExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, WidgetExt};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const TOOLTIP: &str = "Black out other displays while playing";
const ICON: &str = "video-display-symbolic";

thread_local! {
    /// Handle for refreshes that start outside the transport (tech-hold edges, toolbar).
    static ACTIVE_SYNC: RefCell<Option<Rc<BlackoutSync>>> = const { RefCell::new(None) };
}

include!("screen_blackout_tech_hold.rs");

/// No-op until the header control is built (called from the tech-hold unit).
fn refresh_for_hold() {
    let sync = ACTIVE_SYNC.with(|s| s.borrow().clone());
    if let Some(sync) = sync {
        sync.sync();
    }
}

thread_local! {
    /// Last cover decision logged; the `[rhino] blackout:` line prints on changes only.
    static LAST_COVER_LOG: Cell<i8> = const { Cell::new(-1) };
}

/// mpv snapshot shared by the session gate and the decision log: has a real open path, is paused.
#[cfg(target_os = "macos")]
fn mpv_media_state(player: &Rc<RefCell<Option<MpvBundle>>>) -> (bool, bool) {
    player.borrow().as_ref().map_or((false, true), |b| {
        (
            b.mpv.get_property::<String>("path").ok().is_some_and(|s| {
                let t = s.trim();
                !t.is_empty() && t != "null" && t != "undefined"
            }),
            b.mpv.get_property::<bool>("pause").unwrap_or(true),
        )
    })
}

/// Always-on decision log for the multi-monitor covers (report-blackout-stuck triage).
#[cfg(target_os = "macos")]
fn log_cover_decision(
    apply: bool,
    enabled: bool,
    focused: bool,
    screens: usize,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    if LAST_COVER_LOG.with(Cell::get) == apply as i8 {
        return;
    }
    LAST_COVER_LOG.with(|c| c.set(apply as i8));
    let (path, paused) = mpv_media_state(player);
    let (depth, live, age) = tech_hold_diag();
    eprintln!(
        "[rhino] blackout: {} enabled={} focused={} screens={} path={} paused={} hold depth={} live={} age={age:?}",
        if apply { "covers ON;" } else { "covers OFF;" },
        enabled, focused, screens, path, paused, depth, live,
    );
}

/// Shared handle for toolbar wiring and transport-driven refresh.
pub struct BlackoutSync {
    blackout: Rc<RefCell<ScreenBlackout>>,
    win: adw::ApplicationWindow,
    #[cfg(target_os = "macos")]
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
        sync_macos(&self.blackout, &self.win, &self.player, recent_visible);
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
