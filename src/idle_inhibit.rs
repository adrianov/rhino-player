//! [gtk_application_inhibit](https://docs.gtk.org/gtk4/method.Application.inhibit.html) for session
//! idle and suspend (screen dim, screensaver, sleep) while the user is **playing** a file. On
//! GNOME/Wayland, GTK uses the standard mechanism (D-Bus/portal) so the shell respects it.

use gtk::prelude::{GtkApplicationExt, IsA};
use gtk::ApplicationInhibitFlags;

use std::cell::RefCell;
use std::rc::Rc;

use crate::mpv_embed::MpvBundle;

fn flags() -> ApplicationInhibitFlags {
    ApplicationInhibitFlags::IDLE | ApplicationInhibitFlags::SUSPEND
}

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

/// Request or clear inhibit; [cookie] holds the return value of [gtk4::prelude::ApplicationExt::inhibit].
pub fn sync(
    app: &impl IsA<gtk::Application>,
    win: Option<&impl IsA<gtk::Window>>,
    should: bool,
    cookie: &RefCell<Option<u32>>,
) {
    if should {
        if cookie.borrow().is_none() {
            let c = app.inhibit(win, flags(), Some("Video playback"));
            if c != 0 {
                *cookie.borrow_mut() = Some(c);
            }
        }
    } else if let Some(c) = cookie.borrow_mut().take() {
        app.uninhibit(c);
    }
}

/// Always remove inhibit (e.g. before quit). Safe if no cookie.
pub fn clear(app: &impl IsA<gtk::Application>, cookie: &RefCell<Option<u32>>) {
    if let Some(c) = cookie.borrow_mut().take() {
        app.uninhibit(c);
    }
}
