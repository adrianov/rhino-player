fn fs_transition_try_begin(busy: &Cell<bool>) -> bool {
    if busy.get() {
        return false;
    }
    busy.set(true);
    true
}

/// Call from [`gtk::prelude::GtkWindowExt::connect_fullscreened_notify`]: coalesced idle clear after the last notify + settle.
fn fs_transition_note_notify_idle_clear(
    busy: &Rc<Cell<bool>>,
    settle_slot: &Rc<RefCell<Option<glib::SourceId>>>,
) {
    if let Some(id) = settle_slot.borrow_mut().take() {
        id.remove();
    }
    let busy_c = Rc::clone(busy);
    let slot_c = Rc::clone(settle_slot);
    let id = glib::timeout_add_local_once(crate::fullscreen_timing::TRANSITION_SETTLE, move || {
        *slot_c.borrow_mut() = None;
        busy_c.set(false);
    });
    *settle_slot.borrow_mut() = Some(id);
}
