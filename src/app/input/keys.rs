include!("keys_families.rs");
include!("keys_handles.rs");
include!("keys_dispatch.rs");

/// When focus is in a widget that needs unmodified key events (typing, caret moves), let GTK handle
/// keys after our [`gtk::PropagationPhase::Capture`] pass — except [`gtk::gdk::Key::Escape`],
/// which is handled above this check in [`w_in_key_controller`].
fn root_focus_wants_raw_keys(win: &adw::ApplicationWindow) -> bool {
    let Some(fw) = gtk::prelude::RootExt::focus(win) else {
        return false;
    };
    fw.downcast_ref::<gtk::TextView>().is_some()
        || fw.downcast_ref::<gtk::Entry>().is_some()
        || fw.downcast_ref::<gtk::SearchEntry>().is_some()
        || fw.downcast_ref::<gtk::SpinButton>().is_some()
        || fw.downcast_ref::<gtk::PasswordEntry>().is_some()
}

/// GDK **Audio\*** keys: hardware play/pause/stop and prev/next on Linux (and keyboards that expose them via GDK).
#[cfg(not(target_os = "macos"))]
fn propagation_for_media_keys(
    key: gtk::gdk::Key,
    play_key: &PlayToggleCtx,
    nav: &SiblingNavTryRefs,
) -> Option<glib::Propagation> {
    if key == gtk::gdk::Key::AudioPlay || key == gtk::gdk::Key::AudioPause {
        crate::user_action_log::act("media key play/pause");
        let _ = toggle_play_pause(play_key);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioStop {
        crate::user_action_log::act("media key stop");
        media_stop(play_key);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioPrev {
        crate::user_action_log::act("media key previous");
        try_load_sibling_pick(sibling_advance::prev_before_current, "previous", nav);
        return Some(glib::Propagation::Stop);
    }
    if key == gtk::gdk::Key::AudioNext {
        crate::user_action_log::act("media key next");
        try_load_sibling_pick(sibling_advance::next_after_eof, "next", nav);
        return Some(glib::Propagation::Stop);
    }
    None
}

/// macOS hardware keys use [`wire_macos_now_playing_remote`] only (`MPRemoteCommandCenter`).
#[cfg(target_os = "macos")]
#[inline]
fn propagation_for_media_keys(
    _key: gtk::gdk::Key,
    _play_key: &PlayToggleCtx,
    _nav: &SiblingNavTryRefs,
) -> Option<glib::Propagation> {
    None
}

fn w_in_key_controller(ctx: &WindowInputCtx) {
    let d = Rc::new(KeyDispatch::new(ctx));
    let k = gtk::EventControllerKey::new();
    // Capture phase: run before the focused widget (e.g. bottom-bar buttons, scales) so Space /
    // Enter / arrows trigger playback shortcuts instead of GTK's button activation / focus
    // navigation defaults.
    k.set_propagation_phase(gtk::PropagationPhase::Capture);
    let d_key = Rc::clone(&d);
    k.connect_key_pressed(move |_c, key, _code, m| d_key.dispatch(key, m));
    ctx.shell.win.add_controller(k);
}
