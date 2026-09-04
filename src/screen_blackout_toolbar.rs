struct BlackoutToolbar {
    btn: gtk::Button,
    readout: gtk::Label,
}

fn build_blackout_toolbar(enabled: bool) -> BlackoutToolbar {
    let btn = build_blackout_btn();
    let readout = build_blackout_readout();
    btn.set_child(Some(&build_blackout_face(&readout)));
    sync_blackout_btn(&btn, &readout, enabled);

    BlackoutToolbar { btn, readout }
}

fn build_blackout_btn() -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("rp-blackout-mbtn");
    btn.set_hexpand(false);
    btn.set_valign(gtk::Align::Center);
    btn.set_tooltip_text(Some(TOOLTIP));
    btn.set_cursor_from_name(Some("pointer"));
    btn
}

fn build_blackout_readout() -> gtk::Label {
    let readout = gtk::Label::new(None);
    readout.add_css_class("rp-blackout-readout");
    readout.set_xalign(0.0);
    readout.set_valign(gtk::Align::Center);
    readout
}

/// Icon + On/Off readout row inside the button.
fn build_blackout_face(readout: &gtk::Label) -> gtk::Box {
    let img = gtk::Image::from_icon_name(ICON);
    img.set_valign(gtk::Align::Center);

    let face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    face.add_css_class("rp-blackout-face");
    face.set_valign(gtk::Align::Center);
    face.append(&img);
    face.append(readout);
    face
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
    #[cfg(not(target_os = "macos"))]
    let _ = player;
    let enabled = crate::db::load_black_out_screens();
    let blackout = Rc::new(RefCell::new(ScreenBlackout::new(enabled)));
    let BlackoutToolbar { btn, readout } = build_blackout_toolbar(enabled);
    sync_btn_visible(&btn);

    #[cfg(target_os = "macos")]
    let sync = Rc::new(new_blackout_sync(&blackout, win, player, recent, &btn));
    #[cfg(not(target_os = "macos"))]
    let sync = Rc::new(new_blackout_sync(&blackout, win, recent, &btn));
    store_active_sync(&sync);

    connect_blackout_clicked(&sync, &btn, &readout);

    (btn, sync)
}

fn new_blackout_sync(
    blackout: &Rc<RefCell<ScreenBlackout>>,
    win: &adw::ApplicationWindow,
    #[cfg(target_os = "macos")] player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &gtk::Box,
    btn: &gtk::Button,
) -> BlackoutSync {
    BlackoutSync {
        blackout: Rc::clone(blackout),
        win: win.clone(),
        #[cfg(target_os = "macos")]
        player: Rc::clone(player),
        recent: recent.clone(),
        btn: btn.clone(),
        dirty: Cell::new(false),
        scheduled: Cell::new(false),
    }
}

fn store_active_sync(sync: &Rc<BlackoutSync>) {
    // Holds start outside the transport (vf swap, seek burst, chapter scrub) and need this handle.
    ACTIVE_SYNC.with(|s| *s.borrow_mut() = Some(Rc::clone(sync)));
}

fn connect_blackout_clicked(sync: &Rc<BlackoutSync>, btn: &gtk::Button, readout: &gtk::Label) {
    let sync_clk = Rc::clone(sync);
    let btn_clk = btn.clone();
    let ro_clk = readout.clone();
    btn.connect_clicked(move |_| toggle_blackout(&sync_clk, &btn_clk, &ro_clk));
}

/// Focus, continue-grid visibility, and display topology.
pub fn wire_blackout_hooks(sync: &Rc<BlackoutSync>) {
    let sync_act = Rc::clone(sync);
    sync.win.connect_is_active_notify(move |_| {
        sync_act.sync();
    });

    let sync_vis = Rc::clone(sync);
    sync.recent
        .connect_notify_local(Some("visible"), move |_, _| {
            sync_vis.sync();
        });

    #[cfg(target_os = "macos")]
    wire_screen_params_macos(Rc::clone(sync));

    #[cfg(target_os = "macos")]
    wire_nswin_screen_macos(Rc::clone(sync));

    let sync_init = Rc::clone(sync);
    let _ = glib::idle_add_local_once(move || sync_init.sync());
}
