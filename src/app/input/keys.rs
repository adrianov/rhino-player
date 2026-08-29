include!("keys_families.rs");
include!("keys_handles.rs");
include!("keys_dispatch.rs");


/// Plain `q` (no modifiers): quit. Runs only after [`root_focus_wants_raw_keys`] has already
/// returned false — so typing in SearchEntry / other editables never reaches here.
///
/// Platform Cmd/Ctrl+Q stays on [`adw::Application`] accelerators (`<Meta>q` / `<Primary>q`);
/// plain `q` is **not** registered as an accel so GTK cannot quit behind the focus guard.
fn quit_key(
    key: gtk::gdk::Key,
    m: gtk::gdk::ModifierType,
    app: &adw::Application,
) -> Option<glib::Propagation> {
    if key == gtk::gdk::Key::q && m.is_empty() {
        crate::user_action_log::act("key q -> quit");
        app.activate_action("quit", None);
        Some(glib::Propagation::Stop)
    } else {
        None
    }
}

/// When focus is in a widget that needs unmodified key events (typing, caret moves), let GTK handle
/// keys after our [`gtk::PropagationPhase::Capture`] pass — except [`gtk::gdk::Key::Escape`],
/// which is handled above this check in [`w_in_key_controller`].
///
/// Walks ancestors: GtkSearchEntry focuses an inner [`gtk::Text`] (Editable), not the SearchEntry
/// itself — a direct downcast miss lets plain `q` reach [`quit_key`] and quit while typing.
fn root_focus_wants_raw_keys(win: &adw::ApplicationWindow) -> bool {
    let Some(fw) = gtk::prelude::RootExt::focus(win) else {
        return false;
    };
    let mut w: Option<gtk::Widget> = Some(fw);
    while let Some(cur) = w {
        if cur.is::<gtk::Editable>() || cur.downcast_ref::<gtk::TextView>().is_some() {
            return true;
        }
        w = gtk::prelude::WidgetExt::parent(&cur);
    }
    false
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
