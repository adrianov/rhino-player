#[cfg(not(target_os = "macos"))]
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
    drop_glib_source(settle_slot.as_ref());
    let busy_c = Rc::clone(busy);
    let slot_c = Rc::clone(settle_slot);
    #[cfg(target_os = "macos")]
    let delay = std::time::Duration::from_millis(120);
    #[cfg(not(target_os = "macos"))]
    let delay = crate::fullscreen_timing::TRANSITION_SETTLE;
    let id = glib::timeout_add_local_once(delay, move || {
        *slot_c.borrow_mut() = None;
        busy_c.set(false);
    });
    *settle_slot.borrow_mut() = Some(id);
}
