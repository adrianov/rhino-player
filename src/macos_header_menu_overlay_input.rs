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
    let g = header_click_gesture();
    connect_press(&g, &ov.win, &btn);
    connect_release(&g, &ov, idx);
    btn.add_controller(g);
}

/// Primary-button click gesture armed in the capture phase.
fn header_click_gesture() -> gtk::GestureClick {
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    g
}

/// Claim single presses so the opening click does not fall through to the list.
fn connect_press(g: &gtk::GestureClick, win: &adw::ApplicationWindow, btn: &gtk::MenuButton) {
    let win = win.clone();
    let btn = btn.clone();
    g.connect_pressed(move |gesture, n, _, _| {
        if n != 1 || !win.is_fullscreen() {
            return;
        }
        gesture.set_state(gtk::EventSequenceState::Claimed);
        crate::macos_header_menu::on_header_menu_press(&btn);
    });
}

/// Single release toggles the overlay panel for this menu entry.
fn connect_release(g: &gtk::GestureClick, ov: &Rc<HeaderMenuOverlay>, idx: usize) {
    let ov = ov.clone();
    g.connect_released(move |gesture, n, _, _| {
        if n != 1 || !ov.win.is_fullscreen() {
            return;
        }
        claim_release(gesture, &ov, idx);
    });
}

fn claim_release(gesture: &gtk::GestureClick, ov: &Rc<HeaderMenuOverlay>, idx: usize) {
    gesture.set_state(gtk::EventSequenceState::Claimed);
    ov.close_siblings(idx);
    ov.toggle(idx);
    schedule_reposition(ov);
}

/// Re-run placement once the panel child has settled into the overlay.
fn schedule_reposition(ov: &Rc<HeaderMenuOverlay>) {
    let ov_idle = Rc::clone(ov);
    let _ = glib::idle_add_local_once(move || ov_idle.reposition_open());
}

pub(super) fn find_list_box(w: &gtk::Widget) -> Option<gtk::ListBox> {
    if let Ok(list) = w.clone().downcast::<gtk::ListBox>() {
        return Some(list);
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(list) = find_list_box(&c) {
            return Some(list);
        }
        child = c.next_sibling();
    }
    None
}
