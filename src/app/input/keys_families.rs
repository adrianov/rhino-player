// Per-key-family handlers for the capture-phase key controller (`KeyDispatch::dispatch`).
// Each returns `None` when the event belongs to no family of this handler.

/// `C` / `c` with the platform copy modifier: copy the playing file (Finder / file-manager style).
fn copy_playing_path_key(
    key: gtk::gdk::Key,
    m: gtk::gdk::ModifierType,
    p: &Rc<RefCell<Option<MpvBundle>>>,
) -> Option<glib::Propagation> {
    if !((key == gtk::gdk::Key::c || key == gtk::gdk::Key::C) && copy_path_modifier_held(m)) {
        return None;
    }
    if try_copy_playing_path(p) {
        Some(glib::Propagation::Stop)
    } else {
        Some(glib::Propagation::Proceed)
    }
}

/// Enter / KP_Enter / `F` / `f`: toggle fullscreen.
fn fullscreen_entry_key(
    key: gtk::gdk::Key,
    win: &adw::ApplicationWindow,
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    skip: &Rc<Cell<bool>>,
    fs_busy: &Rc<Cell<bool>>,
) -> Option<glib::Propagation> {
    if !(key == gtk::gdk::Key::Return
        || key == gtk::gdk::Key::KP_Enter
        || key == gtk::gdk::Key::f
        || key == gtk::gdk::Key::F)
    {
        return None;
    }
    crate::user_action_log::act("key fullscreen (Enter/F)");
    toggle_fullscreen(win, fr, lu, skip, fs_busy);
    Some(glib::Propagation::Stop)
}

/// `M` / `m`: mute toggle on the live player.
fn mute_toggle_key(
    key: gtk::gdk::Key,
    p: &Rc<RefCell<Option<MpvBundle>>>,
) -> Option<glib::Propagation> {
    if !(key == gtk::gdk::Key::m || key == gtk::gdk::Key::M) {
        return None;
    }
    crate::user_action_log::act("key mute toggle (M)");
    let g = p.borrow();
    let Some(b) = g.as_ref() else {
        return Some(glib::Propagation::Proceed);
    };
    if b
        .mpv
        .set_property(
            "mute",
            !b.mpv.get_property::<bool>("mute").unwrap_or(false),
        )
        .is_err()
    {
        return Some(glib::Propagation::Proceed);
    }
    Some(glib::Propagation::Stop)
}

/// Up / Down: nudge mpv volume.
fn volume_nudge_key(
    key: gtk::gdk::Key,
    p: &Rc<RefCell<Option<MpvBundle>>>,
) -> Option<glib::Propagation> {
    if key != gtk::gdk::Key::Up && key != gtk::gdk::Key::Down {
        return None;
    }
    let up = key == gtk::gdk::Key::Up;
    crate::user_action_log::act(if up {
        "key volume up"
    } else {
        "key volume down"
    });
    let g = p.borrow();
    let Some(b) = g.as_ref() else {
        return Some(glib::Propagation::Proceed);
    };
    nudge_mpv_volume(&b.mpv, if up { 5.0 } else { -5.0 });
    Some(glib::Propagation::Stop)
}

/// Ctrl+Left / Ctrl+Right: load the previous / next sibling file.
fn ctrl_arrow_sibling_key(
    key: gtk::gdk::Key,
    m: gtk::gdk::ModifierType,
    nav: &SiblingNavTryRefs,
) -> Option<glib::Propagation> {
    if !m.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        return None;
    }
    if key == gtk::gdk::Key::Left || key == gtk::gdk::Key::KP_Left {
        crate::user_action_log::act("key Ctrl+Left -> previous file");
        try_load_sibling_pick(sibling_advance::prev_before_current, "previous", nav);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::Right || key == gtk::gdk::Key::KP_Right {
        crate::user_action_log::act("key Ctrl+Right -> next file");
        try_load_sibling_pick(sibling_advance::next_after_eof, "next", nav);
        return Some(glib::Propagation::Stop);
    }
    None
}

/// Space: play/pause toggle.
fn space_play_key(key: gtk::gdk::Key, play_key: &PlayToggleCtx) -> glib::Propagation {
    if key != gtk::gdk::Key::space {
        return glib::Propagation::Proceed;
    }
    crate::user_action_log::act("key Space -> play/pause");
    if !toggle_play_pause(play_key) {
        return glib::Propagation::Proceed;
    }
    glib::Propagation::Stop
}


