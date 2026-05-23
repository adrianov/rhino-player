// Fullscreen overlay: capture MenuButton press and block orphan popover surfaces.

fn wire_btn_fullscreen_block(win: &adw::ApplicationWindow, btn: &gtk::MenuButton) {
    let win2 = win.clone();
    btn.connect_activate(move |b| {
        if win2.is_fullscreen() {
            b.set_active(false);
        }
    });
}

fn wire_popover_fullscreen_guard(win: &adw::ApplicationWindow, pop: &gtk::Popover) {
    let win_map = win.clone();
    pop.connect_map(move |p| {
        if win_map.is_fullscreen() {
            p.popdown();
        }
    });
    let win_show = win.clone();
    pop.connect_show(move |p| {
        if win_show.is_fullscreen() {
            p.popdown();
        }
    });
}

fn wire_btn_press(ov: Rc<HeaderMenuOverlay>, idx: usize, entry: &MenuEntry) {
    let btn = entry.btn.clone();
    let btn_press = btn.clone();
    let ov_press = Rc::clone(&ov);
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    g.connect_pressed(move |gesture, n, _, _| {
        if n != 1 || !ov_press.win.is_fullscreen() {
            return;
        }
        gesture.set_state(gtk::EventSequenceState::Claimed);
        crate::macos_header_menu::on_header_menu_press(&btn_press);
    });
    let ov_rel = Rc::clone(&ov);
    g.connect_released(move |gesture, n, _, _| {
        if n != 1 || !ov_rel.win.is_fullscreen() {
            return;
        }
        gesture.set_state(gtk::EventSequenceState::Claimed);
        ov_rel.close_siblings(idx);
        ov_rel.toggle(idx);
        let ov_idle = Rc::clone(&ov_rel);
        let _ = glib::idle_add_local_once(move || ov_idle.reposition_open());
    });
    btn.add_controller(g);
}
