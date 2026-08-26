include!("speed_pick.rs");
include!("speed_menu_widgets.rs");

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
    let speed_list = build_speed_list();
    let speed_pop = wrap_speed_list_in_popover(&speed_list);
    let speed_mbtn = build_speed_mbtn(&speed_pop);
    let speed_readout = build_speed_readout();
    speed_mbtn.set_child(Some(&build_speed_face(&speed_readout)));

    let speed_sync = Rc::new(Cell::new(false));
    connect_speed_picks(
        SpeedWiringCtx {
            player,
            gl,
            video_pref,
            app,
        },
        &speed_list,
        &speed_pop,
        &speed_mbtn,
        &speed_readout,
        Rc::clone(&speed_sync),
    );
    SpeedMenuResult {
        speed_readout,
        speed_mbtn,
        speed_list,
        speed_sync,
    }
}

fn build_speed_readout() -> gtk::Label {
    let speed_readout = gtk::Label::new(Some(&playback_speed::format_step(1.0)));
    speed_readout.add_css_class("rp-speed-readout");
    speed_readout.set_valign(gtk::Align::Center);
    speed_readout.set_xalign(0.0);
    speed_readout
}

/// Long-lived refs the speed-menu wiring needs (player, gl area, prefs, app).
struct SpeedWiringCtx<'a> {
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
    gl: &'a gtk::GLArea,
    video_pref: &'a Rc<RefCell<db::VideoPrefs>>,
    app: &'a adw::Application,
}

fn connect_speed_picks(
    ctx: SpeedWiringCtx<'_>,
    speed_list: &gtk::ListBox,
    speed_pop: &gtk::Popover,
    speed_mbtn: &gtk::MenuButton,
    speed_readout: &gtk::Label,
    speed_sync: Rc<Cell<bool>>,
) {
    #[cfg(not(target_os = "macos"))]
    let open_pick = Rc::new(Cell::new(false));
    #[cfg(target_os = "macos")]
    let open_pick = macos_arm_speed_pick(speed_mbtn, speed_pop, speed_list);
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::wire_header_menu_trace("speed", speed_mbtn, speed_pop);
    connect_speed_row_signals(
        speed_list,
        SpeedPick {
            player: ctx.player.clone(),
            gl: ctx.gl.clone(),
            video_pref: Rc::clone(ctx.video_pref),
            app: ctx.app.clone(),
            mbtn: speed_mbtn.clone(),
            readout: speed_readout.clone(),
            sync: speed_sync,
            pick: open_pick,
        },
    );
}

/// macOS: open/pick guards so the row signal fires exactly once per deliberate choice.
#[cfg(target_os = "macos")]
fn macos_arm_speed_pick(
    speed_mbtn: &gtk::MenuButton,
    speed_pop: &gtk::Popover,
    speed_list: &gtk::ListBox,
) -> Rc<Cell<bool>> {
    crate::macos_header_menu::wire_menu_btn_open_guard(speed_mbtn);
    let pick = crate::macos_header_menu::arm_menu_list_pick_guard(speed_pop, speed_list);
    crate::macos_header_menu::register_list_pick(pick.clone());
    pick
}

fn connect_speed_row_signals(speed_list: &gtk::ListBox, pick_ctx: SpeedPick) {
    #[cfg(not(target_os = "macos"))]
    speed_list.connect_row_activated(move |list, row| {
        apply_speed_row_pick(&pick_ctx, list, row);
    });
    #[cfg(target_os = "macos")]
    speed_list.connect_row_selected(move |list, row| {
        if let Some(row) = row {
            apply_speed_row_pick(&pick_ctx, list, row);
        }
    });
}

