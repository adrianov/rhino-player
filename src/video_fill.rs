//! Fill-screen toggle: zoom video to fill the display by panning/scanning (mpv `panscan`).
//!
//! The button appears in the header only in fullscreen when the video aspect ratio
//! differs from the screen. `preferred` tracks the user's intent and is restored each
//! time fullscreen is re-entered. Panscan is reset when the button hides (fullscreen exit
//! or media change), but `preferred` is only cleared when new media loads.

use gtk::prelude::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::mpv_embed::MpvBundle;

const ICON: &str = "view-fill-symbolic";
const TOOLTIP: &str = "Fill Screen";
/// Aspect ratio difference below this threshold is treated as "already matching".
const AR_TOLERANCE: f64 = 0.02;

/// Shared state for the fill button.
pub struct FillSync {
    btn: gtk::Button,
    /// Whether panscan is currently applied to mpv.
    active: Cell<bool>,
    /// The user's last explicit choice — restored when re-entering fullscreen.
    preferred: Cell<bool>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
}

impl FillSync {
    /// Recheck visibility; apply or reset panscan to match user preference.
    pub fn sync(&self) {
        let is_fs = self.win.is_fullscreen();
        let mismatch = self.aspect_mismatch();
        let show = is_fs && mismatch;
        if show {
            self.apply_panscan(self.preferred.get());
        } else if self.active.get() {
            self.reset_panscan();
        }
        self.btn.set_visible(show);
        if is_fs && !mismatch {
            if let Some(ar) = monitor_ar(&self.win) {
                eprintln!("[rhino] fill: fullscreen but no AR mismatch (monitor={ar:.3})");
            }
        }
    }

    /// Clear preference on new media so fill doesn't carry over across unrelated videos.
    pub fn reset_preferred(&self) {
        self.preferred.set(false);
        if self.active.get() {
            self.reset_panscan();
        }
        self.btn.set_visible(false);
    }

    fn aspect_mismatch(&self) -> bool {
        let guard = self.player.borrow();
        let Some(b) = guard.as_ref() else { return false };
        let Some(screen_ar) = monitor_ar(&self.win) else { return false };
        let Ok(vw) = b.mpv.get_property::<i64>("dwidth") else { return false };
        let Ok(vh) = b.mpv.get_property::<i64>("dheight") else { return false };
        if vw <= 0 || vh <= 0 {
            return false;
        }
        (screen_ar - vw as f64 / vh as f64).abs() > AR_TOLERANCE
    }

    fn apply_panscan(&self, on: bool) {
        self.active.set(on);
        self.preferred.set(on);
        if let Some(b) = self.player.borrow().as_ref() {
            let v: f64 = if on { 1.0 } else { 0.0 };
            if let Err(e) = b.mpv.set_property("panscan", v) {
                eprintln!("[rhino] fill: panscan set failed: {e}");
            }
        }
        if on {
            self.btn.add_css_class("rp-fill-on");
        } else {
            self.btn.remove_css_class("rp-fill-on");
        }
    }

    fn reset_panscan(&self) {
        self.active.set(false);
        if let Some(b) = self.player.borrow().as_ref() {
            let _ = b.mpv.set_property("panscan", 0.0f64);
        }
        self.btn.remove_css_class("rp-fill-on");
    }
}

/// Returns the aspect ratio of the monitor the window is currently on.
/// Uses GDK monitor geometry so it's available immediately, even during fullscreen transition.
fn monitor_ar(win: &adw::ApplicationWindow) -> Option<f64> {
    use gtk::prelude::NativeExt;
    let surface = win.surface()?;
    let monitor = gtk::prelude::WidgetExt::display(win).monitor_at_surface(&surface)?;
    let geo = monitor.geometry();
    let (w, h) = (geo.width(), geo.height());
    (w > 0 && h > 0).then(|| w as f64 / h as f64)
}

/// Build the fill header button and wire fullscreen + transport resync.
pub fn build_fill_header(
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) -> (gtk::Button, Rc<FillSync>) {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("rp-fill-btn");
    btn.set_valign(gtk::Align::Center);
    btn.set_cursor_from_name(Some("pointer"));
    btn.set_tooltip_text(Some(TOOLTIP));
    btn.set_visible(false);

    if let Some(display) = gtk::gdk::Display::default() {
        if !gtk::IconTheme::for_display(&display).has_icon(ICON) {
            eprintln!("[rhino] fill: icon not found in theme: {ICON}");
        }
    }
    let img = gtk::Image::from_icon_name(ICON);
    img.set_valign(gtk::Align::Center);
    btn.set_child(Some(&img));

    let sync = Rc::new(FillSync {
        btn: btn.clone(),
        active: Cell::new(false),
        preferred: Cell::new(false),
        player: Rc::clone(player),
        win: win.clone(),
    });

    let sc = Rc::clone(&sync);
    btn.connect_clicked(move |_| sc.apply_panscan(!sc.preferred.get()));

    // Defer sync: window dimensions are updated after fullscreened-notify fires.
    let sw = Rc::clone(&sync);
    win.connect_fullscreened_notify(move |_| {
        let s = Rc::clone(&sw);
        let _ = glib::idle_add_local_once(move || s.sync());
    });

    let st = Rc::clone(&sync);
    FILL_RESYNC.with(|s| *s.borrow_mut() = Some(Rc::new(move || st.sync())));

    let sr = Rc::clone(&sync);
    FILL_RESET.with(|s| *s.borrow_mut() = Some(Rc::new(move || sr.reset_preferred())));

    (btn, sync)
}

thread_local! {
    static FILL_RESYNC: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static FILL_RESET: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Called on `VideoReconfig` / `FileLoaded` to recheck fill button visibility.
pub fn request_fill_resync() {
    FILL_RESYNC.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}

/// Called on `PathChanged` (new media) to clear the fill preference.
pub fn request_fill_reset() {
    FILL_RESET.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}
