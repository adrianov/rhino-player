//! Black out non-video displays while playing (macOS). See `docs/features/17-window-behavior.md`.

use crate::mpv_embed::MpvBundle;
use glib::prelude::ObjectExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, WidgetExt};
use std::cell::RefCell;
use std::rc::Rc;

const TOOLTIP: &str = "Black out other displays while playing";
const ICON: &str = "video-display-symbolic";

/// Shared handle for toolbar wiring and transport-driven resync.
pub struct BlackoutSync {
    blackout: Rc<RefCell<ScreenBlackout>>,
    win: adw::ApplicationWindow,
    player: Rc<RefCell<Option<MpvBundle>>>,
    recent: gtk::Box,
}

impl BlackoutSync {
    pub fn sync(&self) {
        let recent_visible = self.recent.is_visible();
        self.blackout
            .borrow_mut()
            .sync(&self.win, &self.player, recent_visible);
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
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        crate::db::save_black_out_screens(on);
    }

    /// Apply or remove overlays from preference, focus, playback, and display topology.
    pub fn sync(
        &mut self,
        win: &adw::ApplicationWindow,
        player: &Rc<RefCell<Option<MpvBundle>>>,
        recent_visible: bool,
    ) {
        #[cfg(target_os = "macos")]
        sync_macos(self, win, player, recent_visible);
        #[cfg(not(target_os = "macos"))]
        let _ = (win, player, recent_visible);
    }
}

#[cfg(target_os = "macos")]
fn should_apply(
    bo: &ScreenBlackout,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent_visible: bool,
) -> bool {
    bo.enabled
        && win.is_active()
        && multi_screen()
        && crate::idle_inhibit::should_inhibit(player, recent_visible)
}

struct BlackoutToolbar {
    btn: gtk::Button,
    readout: gtk::Label,
}

fn build_blackout_toolbar(enabled: bool) -> BlackoutToolbar {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("rp-blackout-mbtn");
    btn.set_hexpand(false);
    btn.set_valign(gtk::Align::Center);
    btn.set_tooltip_text(Some(TOOLTIP));
    btn.set_cursor_from_name(Some("pointer"));

    let img = gtk::Image::from_icon_name(ICON);
    img.set_valign(gtk::Align::Center);

    let readout = gtk::Label::new(None);
    readout.add_css_class("rp-blackout-readout");
    readout.set_xalign(0.0);
    readout.set_valign(gtk::Align::Center);

    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    face.add_css_class("rp-blackout-face");
    face.set_valign(gtk::Align::Center);
    face.append(&img);
    face.append(&readout);
    btn.set_child(Some(&face));
    sync_blackout_btn(&btn, &readout, enabled);

    BlackoutToolbar { btn, readout }
}

fn sync_blackout_btn(btn: &gtk::Button, readout: &gtk::Label, on: bool) {
    readout.set_label(if on { "On" } else { "Off" });
    if on {
        btn.add_css_class("rp-blackout-on");
    } else {
        btn.remove_css_class("rp-blackout-on");
    }
}

fn sync_btn_visible(btn: &gtk::Button) {
    btn.set_visible(multi_screen());
}

fn toggle_blackout(sync: &Rc<BlackoutSync>, btn: &gtk::Button, readout: &gtk::Label) {
    let on = {
        let mut b = sync.blackout.borrow_mut();
        let next = !b.enabled();
        b.set_enabled(next);
        next
    };
    sync_blackout_btn(btn, readout, on);
    sync.sync();
}

/// Build header control and return the shared sync handle (hooks wired separately).
pub fn build_blackout_header(
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &gtk::Box,
) -> (gtk::Button, Rc<BlackoutSync>) {
    let enabled = crate::db::load_black_out_screens();
    let blackout = Rc::new(RefCell::new(ScreenBlackout::new(enabled)));
    let BlackoutToolbar { btn, readout } = build_blackout_toolbar(enabled);
    sync_btn_visible(&btn);

    let sync = Rc::new(BlackoutSync {
        blackout: Rc::clone(&blackout),
        win: win.clone(),
        player: Rc::clone(player),
        recent: recent.clone(),
    });

    let sync_clk = Rc::clone(&sync);
    let btn_clk = btn.clone();
    let ro_clk = readout.clone();
    btn.connect_clicked(move |_| toggle_blackout(&sync_clk, &btn_clk, &ro_clk));

    (btn, sync)
}

/// Focus, display topology, and native window screen moves.
pub fn wire_blackout_hooks(sync: &Rc<BlackoutSync>, btn: &gtk::Button) {
    let sync_act = Rc::clone(sync);
    sync.win.connect_is_active_notify(move |_| {
        sync_act.sync();
    });

    let sync_vis = Rc::clone(sync);
    let btn_vis = btn.clone();
    sync.recent.connect_notify_local(Some("visible"), move |_, _| {
        sync_btn_visible(&btn_vis);
        sync_vis.sync();
    });

    #[cfg(target_os = "macos")]
    wire_screen_params_macos(Rc::clone(sync), btn.clone());

    #[cfg(target_os = "macos")]
    wire_nswin_screen_macos(Rc::clone(sync));

    let sync_init = Rc::clone(sync);
    let _ = glib::idle_add_local_once(move || sync_init.sync());
}

#[cfg(target_os = "macos")]
include!("screen_blackout_macos.rs");
