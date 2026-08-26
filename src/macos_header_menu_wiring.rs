// Popover / menu surface wiring: outside-dismiss guard and the list pick-guard family.

/// Before the popover surface exists: block outside dismiss + compositing refresh.
pub fn wire_menu_btn_open_guard(btn: &gtk::MenuButton) {
    let btn = btn.clone();
    let g = gtk::GestureClick::new();
    g.set_button(gtk::gdk::BUTTON_PRIMARY);
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    let btn_press = btn.clone();
    g.connect_pressed(move |_, n, _, _| {
        if n == 1 {
            on_header_menu_press(&btn_press);
        }
    });
    btn.add_controller(g);
}

/// Speed list: block spurious selection while the opening click settles (theater toolbar).
pub fn arm_menu_list_pick_guard(pop: &gtk::Popover, list: &gtk::ListBox) -> Rc<Cell<bool>> {
    let block = Rc::new(Cell::new(false));
    let b_map = block.clone();
    let list_map = list.clone();
    pop.connect_map(move |_| {
        arm_pick_block(&b_map);
        freeze_list_during_hold(&list_map);
    });
    let b_show = block.clone();
    let list_show = list.clone();
    pop.connect_show(move |_| {
        arm_pick_block(&b_show);
        freeze_list_during_hold(&list_show);
    });
    block
}

thread_local! {
    static LIST_PICK: RefCell<Option<Rc<Cell<bool>>>> = const { RefCell::new(None) };
}

pub fn register_list_pick(block: Rc<Cell<bool>>) {
    LIST_PICK.with(|s| *s.borrow_mut() = Some(block));
}

/// Raise the pick guard for one menu-hold window.
fn arm_pick_block(block: &Rc<Cell<bool>>) {
    block.set(true);
    let b2 = block.clone();
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
        move || b2.set(false),
    );
}

/// Freeze a speed list while the opening click settles.
fn freeze_list_during_hold(list: &gtk::ListBox) {
    list.set_sensitive(false);
    let list = list.clone();
    let _ = glib::timeout_add_local_once(
        std::time::Duration::from_millis(u64::from(MENU_HOLD_MS)),
        move || list.set_sensitive(true),
    );
}

fn arm_list_pick() {
    LIST_PICK.with(|s| {
        if let Some(block) = s.borrow().as_ref() {
            arm_pick_block(block);
        }
    });
}

/// Fullscreen overlay open: same pick guard as popover map/show (windowed).
pub fn arm_list_pick_on_open(list: &gtk::ListBox) {
    freeze_list_during_hold(list);
    arm_list_pick();
}
