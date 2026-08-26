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

mod fill_sync;

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
    let btn = build_fill_button();
    let sync = Rc::new(FillSync {
        btn: btn.clone(),
        active: Cell::new(false),
        preferred: Cell::new(false),
        player: Rc::clone(player),
        win: win.clone(),
    });
    connect_fill_clicked(&btn, &sync);
    connect_fill_resync_on_fullscreen(win, &sync);
    register_fill_hooks(&sync);
    (btn, sync)
}

fn build_fill_button() -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("rp-fill-btn");
    btn.set_valign(gtk::Align::Center);
    btn.set_cursor_from_name(Some("pointer"));
    btn.set_tooltip_text(Some(TOOLTIP));
    btn.set_visible(false);

    warn_missing_fill_icon();
    let img = gtk::Image::from_icon_name(ICON);
    img.set_valign(gtk::Align::Center);
    btn.set_child(Some(&img));
    btn
}

fn warn_missing_fill_icon() {
    if let Some(display) = gtk::gdk::Display::default() {
        if !gtk::IconTheme::for_display(&display).has_icon(ICON) {
            eprintln!("[rhino] fill: icon not found in theme: {ICON}");
        }
    }
}

fn connect_fill_clicked(btn: &gtk::Button, sync: &Rc<FillSync>) {
    let sc = Rc::clone(sync);
    btn.connect_clicked(move |_| {
        let on = !sc.preferred.get();
        crate::user_action_log::act(format!(
            "fill screen button -> {}",
            if on { "on" } else { "off" }
        ));
        sc.apply_panscan(on);
    });
}

fn connect_fill_resync_on_fullscreen(win: &adw::ApplicationWindow, sync: &Rc<FillSync>) {
    // Defer sync: window dimensions are updated after fullscreened-notify fires.
    let sw = Rc::clone(sync);
    win.connect_fullscreened_notify(move |_| {
        let s = Rc::clone(&sw);
        let _ = glib::idle_add_local_once(move || s.sync());
    });
}

fn register_fill_hooks(sync: &Rc<FillSync>) {
    let st = Rc::clone(sync);
    FILL_RESYNC.with(|s| *s.borrow_mut() = Some(Rc::new(move || st.sync())));
    let sr = Rc::clone(sync);
    FILL_RESET.with(|s| *s.borrow_mut() = Some(Rc::new(move || sr.reset_preferred())));
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
