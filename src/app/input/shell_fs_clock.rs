// Fullscreen wall-clock label: one ticking source while fullscreen, stopped on leave.

fn refresh_fs_wall_clock(lbl: &gtk::Label) {
    lbl.set_label(format_wall_clock_now().as_str());
}

fn stop_fs_clock_tick(slot: &Rc<RefCell<Option<glib::SourceId>>>) {
    drop_glib_source(slot.as_ref());
}

fn fs_clock_timer_step(
    wo: &adw::ApplicationWindow,
    tick_slot: &Rc<RefCell<Option<glib::SourceId>>>,
    lbl: &gtk::Label,
) -> glib::ControlFlow {
    if !wo.is_fullscreen() {
        stop_fs_clock_tick(tick_slot);
        glib::ControlFlow::Break
    } else {
        refresh_fs_wall_clock(lbl);
        glib::ControlFlow::Continue
    }
}

fn show_fs_wall_clock_fullscreen(
    lbl: &gtk::Label,
    tick_slot: &Rc<RefCell<Option<glib::SourceId>>>,
    win: &adw::ApplicationWindow,
) {
    refresh_fs_wall_clock(lbl);
    lbl.set_visible(true);
    stop_fs_clock_tick(tick_slot);
    let fc = lbl.clone();
    let fts = tick_slot.clone();
    let wo = win.clone();
    let id = glib::timeout_add_seconds_local(1, move || fs_clock_timer_step(&wo, &fts, &fc));
    *tick_slot.borrow_mut() = Some(id);
}
