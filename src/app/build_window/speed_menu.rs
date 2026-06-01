fn dismiss_speed_menu(btn: &gtk::MenuButton) {
    #[cfg(target_os = "macos")]
    {
        btn.remove_css_class("rp-header-menu-open");
        crate::macos_header_menu_overlay::overlay_close_all("speed_pick");
    }
    if let Some(pop) = btn.popover() {
        pop.popdown();
    }
    btn.set_active(false);
}

fn speed_row_index(list: &gtk::ListBox, row: &gtk::ListBoxRow) -> u32 {
    (0i32..playback_speed::SPEEDS.len() as i32)
        .find(|&ix| list.row_at_index(ix).is_some_and(|r| r == *row))
        .unwrap_or(0) as u32
}

/// Everything a speed-row pick needs; cloned once into the row signal handler.
#[derive(Clone)]
struct SpeedPick {
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    mbtn: gtk::MenuButton,
    readout: gtk::Label,
    sync: Rc<Cell<bool>>,
    pick: Rc<Cell<bool>>,
}

fn apply_speed_row_pick(c: &SpeedPick, list: &gtk::ListBox, row: &gtk::ListBoxRow) {
    if c.sync.get() || c.pick.get() || !list.is_sensitive() {
        #[cfg(target_os = "macos")]
        crate::macos_header_menu_debug::log_event(
            "speed",
            "row_pick_skip",
            &format!(
                "sync={} pick={} sensitive={}",
                c.sync.get(),
                c.pick.get(),
                list.is_sensitive()
            ),
        );
        return;
    }
    let v = playback_speed::value_at(speed_row_index(list, row));
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::log_event("speed", "row_apply", &format!("rate={v}"));
    let guard = c.player.borrow();
    let Some(b) = guard.as_ref() else {
        eprintln!("[rhino] speed: row pick with no player bundle");
        return;
    };
    if b.mpv.set_property("speed", v).is_err() {
        eprintln!("[rhino] speed: set_property speed={v} failed");
        return;
    }
    drop(guard);
    playback_speed::stamp_header(&c.mbtn, &c.readout, v);
    dismiss_speed_menu(&c.mbtn);
    c.gl.queue_render();
    let player_idle = Rc::clone(&c.player);
    let vp_idle = Rc::clone(&c.video_pref);
    let app_idle = c.app.clone();
    let _ = glib::idle_add_local_once(move || {
        let guard = player_idle.borrow();
        let Some(pl) = guard.as_ref() else {
            return;
        };
        let r = video_pref::refresh_smooth_for_playback_speed(pl, &mut vp_idle.borrow_mut(), Some(v));
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&app_idle);
        }
    });
}

/// Builds the playback-speed popover; icon + rate caption share one [`gtk::MenuButton`] hit target
/// (horizontal row keeps header / fullscreen toolbar row height unchanged).
struct SpeedMenuResult {
    speed_readout: gtk::Label,
    speed_mbtn: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_sync: Rc<Cell<bool>>,
}

fn build_speed_menu(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    gl: &gtk::GLArea,
    video_pref: &Rc<RefCell<db::VideoPrefs>>,
    app: &adw::Application,
) -> SpeedMenuResult {
    let speed_list = gtk::ListBox::new();
    // Linux: row-activated on single click (GTK does not reliably apply speed via row-selected
    // when activate-on-single-click is false). macOS: row-selected + false avoids spurious apply
    // while the opening click settles (pick guard).
    #[cfg(not(target_os = "macos"))]
    speed_list.set_activate_on_single_click(true);
    #[cfg(target_os = "macos")]
    speed_list.set_activate_on_single_click(false);
    speed_list.add_css_class("rich-list");
    for s in &playback_speed::SPEEDS {
        let row = gtk::ListBoxRow::new();
        let lab = gtk::Label::new(Some(&playback_speed::format_step(*s)));
        lab.set_halign(gtk::Align::Start);
        lab.set_margin_start(10);
        lab.set_margin_end(10);
        lab.set_margin_top(6);
        lab.set_margin_bottom(6);
        row.set_child(Some(&lab));
        speed_list.append(&row);
    }
    let speed_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .max_content_height(crate::header_menu_scroll::SPEED_MAX_H)
        .child(&speed_list)
        .build();
    speed_scrl.add_css_class(crate::header_menu_scroll::SCROLL_CLASS_SPEED);
    let speed_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    speed_col.add_css_class("rp-popover-box");
    speed_col.append(&speed_scrl);
    let speed_pop = gtk::Popover::new();
    speed_pop.add_css_class("rp-header-popover");
    speed_pop.set_child(Some(&speed_col));
    header_popover_non_modal(&speed_pop);
    #[cfg(target_os = "macos")]
    {
        speed_pop.set_has_arrow(false);
        crate::macos_header_menu::wire_popover(&speed_pop);
    }
    let speed_mbtn = gtk::MenuButton::new();
    speed_mbtn.set_popover(Some(&speed_pop));
    speed_mbtn.set_tooltip_text(Some("Playback speed"));
    speed_mbtn.set_sensitive(false);
    speed_mbtn.add_css_class("flat");
    speed_mbtn.add_css_class("rp-speed-mbtn");
    speed_mbtn.set_hexpand(false);
    speed_mbtn.set_valign(gtk::Align::Center);
    speed_mbtn.set_always_show_arrow(false);

    let speed_readout = gtk::Label::new(Some(&playback_speed::format_step(1.0)));
    speed_readout.add_css_class("rp-speed-readout");
    speed_readout.set_valign(gtk::Align::Center);
    speed_readout.set_xalign(0.0);

    let speed_icon = gtk::Image::from_icon_name("speedometer-symbolic");
    speed_icon.set_valign(gtk::Align::Center);

    let speed_face = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    speed_face.add_css_class("rp-speed-face");
    speed_face.set_valign(gtk::Align::Center);
    speed_face.append(&speed_icon);
    speed_face.append(&speed_readout);
    speed_mbtn.set_child(Some(&speed_face));

    let speed_sync = Rc::new(Cell::new(false));
    #[cfg(not(target_os = "macos"))]
    let open_pick = Rc::new(Cell::new(false));
    #[cfg(target_os = "macos")]
    let open_pick = {
        crate::macos_header_menu::wire_menu_btn_open_guard(&speed_mbtn);
        let pick = crate::macos_header_menu::arm_menu_list_pick_guard(&speed_pop, &speed_list);
        crate::macos_header_menu::register_list_pick(pick.clone());
        pick
    };
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("speed", &speed_mbtn, &speed_pop);
    let pick_ctx = SpeedPick {
        player: player.clone(),
        gl: gl.clone(),
        video_pref: Rc::clone(video_pref),
        app: app.clone(),
        mbtn: speed_mbtn.clone(),
        readout: speed_readout.clone(),
        sync: speed_sync.clone(),
        pick: open_pick,
    };
    #[cfg(not(target_os = "macos"))]
    speed_list.connect_row_activated(move |list, row| apply_speed_row_pick(&pick_ctx, list, row));
    #[cfg(target_os = "macos")]
    speed_list.connect_row_selected(move |list, row| {
        if let Some(row) = row {
            apply_speed_row_pick(&pick_ctx, list, row);
        }
    });
    SpeedMenuResult {
        speed_readout,
        speed_mbtn,
        speed_list,
        speed_sync,
    }
}
