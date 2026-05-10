//! Idle / display-sleep inhibition while the user is **playing** (see [`should_inhibit`]).
//!
//! **Linux**: [`gtk::Application::inhibit`](https://docs.gtk.org/gtk4/method.Application.inhibit.html)
//! with IDLE + SUSPEND (D‑Bus / portal — GNOME dims and sleeps respect it).
//! **macOS**: `NSProcessInfo` activity with idle display/system sleep disabled — see
//! [Apple: beginActivity(withOptions:reason:)](https://developer.apple.com/documentation/foundation/nsprocessinfo/beginactivitywithoptions).
//! disables idle display and system sleep (`IdleDisplaySleepDisabled` + `IdleSystemSleepDisabled`); GTK inhibit is not relied on.

use std::cell::RefCell;
use std::rc::Rc;

use crate::mpv_embed::MpvBundle;

#[cfg(not(target_os = "macos"))]
pub type Held = u32;

#[cfg(target_os = "macos")]
pub type Held =
    objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>;

/// True when a file is loaded, **not** paused, and the **continue** grid is hidden (we are in playback).
pub fn should_inhibit(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_scroller_visible: bool,
) -> bool {
    if recent_scroller_visible {
        return false;
    }
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        return false;
    };
    if b.mpv.get_property::<bool>("pause").unwrap_or(true) {
        return false;
    }
    b.mpv.get_property::<String>("path").ok().is_some_and(|s| {
        let t = s.trim();
        !t.is_empty() && t != "null" && t != "undefined"
    })
}

#[cfg(not(target_os = "macos"))]
use gtk::prelude::{GtkApplicationExt, IsA};

#[cfg(not(target_os = "macos"))]
fn gtk_inhibit_flags() -> gtk::ApplicationInhibitFlags {
    gtk::ApplicationInhibitFlags::IDLE | gtk::ApplicationInhibitFlags::SUSPEND
}

/// Request or clear inhibit; [`RefCell`] holds the platform token returned when active.
#[cfg(not(target_os = "macos"))]
pub fn sync(
    app: &impl IsA<gtk::Application>,
    win: Option<&impl IsA<gtk::Window>>,
    should: bool,
    cookie: &RefCell<Option<Held>>,
) {
    if should {
        if cookie.borrow().is_none() {
            let c = app.inhibit(win, gtk_inhibit_flags(), Some("Video playback"));
            if c != 0 {
                *cookie.borrow_mut() = Some(c);
            }
        }
    } else if let Some(c) = cookie.borrow_mut().take() {
        app.uninhibit(c);
    }
}

#[cfg(target_os = "macos")]
pub fn sync(
    app: &impl gtk::prelude::IsA<gtk::Application>,
    win: Option<&impl gtk::prelude::IsA<gtk::Window>>,
    should: bool,
    cookie: &RefCell<Option<Held>>,
) {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let _ = app;
    let _ = win;
    if should {
        if cookie.borrow().is_none() {
            let info = NSProcessInfo::processInfo();
            let opts = NSActivityOptions::IdleDisplaySleepDisabled
                .union(NSActivityOptions::IdleSystemSleepDisabled);
            let reason = NSString::from_str("Video playback");
            let act = info.beginActivityWithOptions_reason(opts, &reason);
            *cookie.borrow_mut() = Some(act);
        }
    } else if let Some(a) = cookie.borrow_mut().take() {
        let info = NSProcessInfo::processInfo();
        unsafe { info.endActivity(&a) };
    }
}

#[cfg(not(target_os = "macos"))]
pub fn clear(app: &impl IsA<gtk::Application>, cookie: &RefCell<Option<Held>>) {
    if let Some(c) = cookie.borrow_mut().take() {
        app.uninhibit(c);
    }
}

#[cfg(target_os = "macos")]
pub fn clear(app: &impl gtk::prelude::IsA<gtk::Application>, cookie: &RefCell<Option<Held>>) {
    use objc2_foundation::NSProcessInfo;
    let _ = app;
    let Some(activity) = cookie.borrow_mut().take() else {
        return;
    };
    let info = NSProcessInfo::processInfo();
    unsafe { info.endActivity(&activity) };
}
