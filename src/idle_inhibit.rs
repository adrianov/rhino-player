//! Idle / display-sleep inhibition while the user is **playing** (see [`should_inhibit`]).
//!
//! **Linux**: [`gtk::Application::inhibit`](https://docs.gtk.org/gtk4/method.Application.inhibit.html)
//! with IDLE + SUSPEND (D‑Bus / portal — GNOME dims and sleeps respect it).
//! **macOS**: an IOKit power assertion (`PreventUserIdleDisplaySleep`) — see
//! [Apple: IOPMAssertionCreateWithName](https://developer.apple.com/documentation/iokit/1557134-iopmassertioncreatewithname).
//! GTK inhibit is not relied on.
//!
//! Do **not** go back to `NSProcessInfo::beginActivityWithOptions` here. Taking an activity re-enters
//! the run loop underneath us, and because this runs from a GLib source, the next dispatch aborts with
//! `g_main_dispatch: assertion failed: (source)` in release builds. IOKit assertions are a plain C call
//! and leave the loop alone.

use std::cell::RefCell;
use std::rc::Rc;

use crate::mpv_embed::MpvBundle;

/// Linux: GTK inhibit cookie. macOS: IOKit assertion id.
pub type Held = u32;

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
mod iokit {
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    const ASSERT_ON: u32 = 255;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: *const c_void,
            level: u32,
            name: *const c_void,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    /// `NSString` is toll-free bridged to `CFStringRef`, so the object pointer is the argument.
    fn cf_string(s: &NSString) -> *const c_void {
        (s as *const NSString).cast()
    }

    /// Assertion id, or `None` when IOKit refused it (`kIOReturnSuccess` is 0).
    pub fn take() -> Option<u32> {
        let kind = NSString::from_str("PreventUserIdleDisplaySleep");
        let name = NSString::from_str("Video playback");
        let mut id = 0u32;
        let rc = unsafe {
            IOPMAssertionCreateWithName(
                cf_string(&kind),
                ASSERT_ON,
                cf_string(&name),
                &mut id as *mut u32,
            )
        };
        if rc != 0 {
            eprintln!("[rhino] idle: IOPMAssertionCreateWithName failed rc={rc}");
            return None;
        }
        Some(id)
    }

    pub fn release(id: u32) {
        let rc = unsafe { IOPMAssertionRelease(id) };
        if rc != 0 {
            eprintln!("[rhino] idle: IOPMAssertionRelease failed rc={rc}");
        }
    }
}

/// Request or clear the display-sleep assertion; [`RefCell`] holds the id while active.
#[cfg(target_os = "macos")]
pub fn sync(
    app: &impl gtk::prelude::IsA<gtk::Application>,
    win: Option<&impl gtk::prelude::IsA<gtk::Window>>,
    should: bool,
    cookie: &RefCell<Option<Held>>,
) {
    let _ = (app, win);
    if should {
        if cookie.borrow().is_none() {
            *cookie.borrow_mut() = iokit::take();
        }
    } else if let Some(id) = cookie.borrow_mut().take() {
        iokit::release(id);
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
    let _ = app;
    if let Some(id) = cookie.borrow_mut().take() {
        iokit::release(id);
    }
}
