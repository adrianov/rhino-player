//! Fill toggle: zoom video to cover the **viewport** (`panscan`) and crop baked-in
//! black strips (`video-crop` from a short `cropdetect` probe).
//!
//! The header button shows when the video surface aspect differs from the content
//! aspect (strip crop when known, else decode size). Call [`bind_fill_viewport`]
//! once the shell mounts the video `GLArea`. `preferred` tracks the user's intent
//! across viewport size / fullscreen changes, and is re-read from the per-video
//! `media.fill_screen` choice when new media opens.

use gtk::prelude::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::mpv_embed::MpvBundle;

mod fill_sync;

use crate::black_bars::BarProbe;

const ICON: &str = "view-fill-symbolic";
const TOOLTIP: &str = "Fill Screen";
/// Aspect ratio difference below this threshold is treated as "already matching".
const AR_TOLERANCE: f64 = 0.02;

/// Shared state for the fill button.
pub struct FillSync {
    btn: gtk::Button,
    /// Whether fill (panscan / bar crop) is currently applied to mpv.
    active: Cell<bool>,
    /// The user's last explicit choice — restored when the button can show again.
    preferred: Cell<bool>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    /// Video surface used for viewport aspect (set by [`bind_fill_viewport`]).
    viewport: RefCell<Option<gtk::GLArea>>,
    resize_hooked: Cell<bool>,
    bars: Rc<BarProbe>,
}

/// Aspect ratio of the video surface (`GLArea` allocation).
fn viewport_ar(viewport: &gtk::GLArea) -> Option<f64> {
    let w = viewport.width();
    let h = viewport.height();
    (w > 0 && h > 0).then(|| w as f64 / h as f64)
}

/// Build the fill header button and wire fullscreen + transport resync.
/// Pair with [`bind_fill_viewport`] after the video surface is created.
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
        viewport: RefCell::new(None),
        resize_hooked: Cell::new(false),
        bars: Rc::new(BarProbe::new()),
    });
    connect_fill_clicked(&btn, &sync);
    connect_fullscreen_resync(win, &sync);
    register_fill_hooks(&sync);
    (btn, sync)
}

/// Attach the video surface so fill can compare viewport vs content aspect.
pub fn bind_fill_viewport(viewport: &gtk::GLArea) {
    FILL_SYNC.with(|c| {
        let Some(sync) = c.borrow().clone() else {
            eprintln!("[rhino] fill: bind_fill_viewport before build_fill_header");
            return;
        };
        sync.attach_viewport(viewport);
    });
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
        sc.apply_fill(on);
        if let Some(path) = current_local_media_path(&sc.player) {
            crate::db::media_save_fill_screen(&path, on);
        }
    });
}

/// Local path of the media currently open in mpv (`None` for streams / no media).
fn current_local_media_path(player: &Rc<RefCell<Option<MpvBundle>>>) -> Option<std::path::PathBuf> {
    let g = player.borrow();
    let b = g.as_ref()?;
    crate::media_probe::local_file_from_mpv(&b.mpv)
}

/// Per-video remembered choice for the currently open media (fitted default when unset).
fn stored_fill_preference(player: &Rc<RefCell<Option<MpvBundle>>>) -> bool {
    current_local_media_path(player)
        .and_then(|p| crate::db::media_fill_screen(&p))
        .unwrap_or(false)
}

fn connect_fullscreen_resync(win: &adw::ApplicationWindow, sync: &Rc<FillSync>) {
    let sw = Rc::clone(sync);
    win.connect_fullscreened_notify(move |_| {
        let s = Rc::clone(&sw);
        let _ = glib::idle_add_local_once(move || s.sync());
    });
}

fn register_fill_hooks(sync: &Rc<FillSync>) {
    FILL_SYNC.with(|c| *c.borrow_mut() = Some(Rc::clone(sync)));
    hook_resync(sync);
    hook_sync_only(sync);
    hook_reset(sync);
}

fn hook_resync(sync: &Rc<FillSync>) {
    let s = Rc::clone(sync);
    FILL_RESYNC.with(|c| *c.borrow_mut() = Some(Rc::new(move || s.on_media_ready())));
}

fn hook_sync_only(sync: &Rc<FillSync>) {
    let s = Rc::clone(sync);
    FILL_SYNC_ONLY.with(|c| *c.borrow_mut() = Some(Rc::new(move || s.sync())));
}

fn hook_reset(sync: &Rc<FillSync>) {
    let s = Rc::clone(sync);
    FILL_RESET.with(|c| *c.borrow_mut() = Some(Rc::new(move || s.reset_preferred())));
}

thread_local! {
    static FILL_SYNC: RefCell<Option<Rc<FillSync>>> = const { RefCell::new(None) };
    static FILL_RESYNC: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static FILL_SYNC_ONLY: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static FILL_RESET: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Called on `VideoReconfig` / `FileLoaded` to recheck fill and (re)try strip detection.
pub fn request_fill_resync() {
    FILL_RESYNC.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}

/// Visibility / apply only (strip probe finished — do not restart detection).
pub(crate) fn request_fill_sync_only() {
    FILL_SYNC_ONLY.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}

/// Called on `PathChanged` (new media) to clear the fill preference and bar probe.
pub fn request_fill_reset() {
    FILL_RESET.with(|s| {
        if let Some(f) = s.borrow().as_ref() {
            f();
        }
    });
}
