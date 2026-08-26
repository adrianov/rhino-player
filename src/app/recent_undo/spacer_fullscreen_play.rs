fn wire_recent_spacer_fullscreen(
    sp_empty: [gtk::Box; 2],
    win: &adw::ApplicationWindow,
    fs: &FullscreenToggleRefs,
    recent: &gtk::Box,
) {
    for sp in &sp_empty {
        wire_spacer_double_click(sp, win, fs, recent);
    }
}

/// Double-click on an empty grid spacer toggles fullscreen (browse overlay visible only).
fn wire_spacer_double_click(
    sp: &gtk::Box,
    win: &adw::ApplicationWindow,
    fs: &FullscreenToggleRefs,
    recent: &gtk::Box,
) {
    let d2 = gtk::GestureClick::new();
    d2.set_button(gtk::gdk::BUTTON_PRIMARY);
    let w2 = win.clone();
    let fr2 = Rc::clone(&fs.fs_restore);
    let lu2 = Rc::clone(&fs.last_unmax);
    let sk2 = Rc::clone(&fs.skip_max_to_fs);
    let fb2 = Rc::clone(&fs.fs_transition_busy);
    let rec2 = recent.clone();
    d2.connect_pressed(move |_, n_press, _, _| {
        if n_press != 2 || !rec2.is_visible() {
            return;
        }
        toggle_fullscreen(&w2, &fr2, &lu2, &sk2, fb2.as_ref());
    });
    sp.add_controller(d2);
}

/// Loaded media pause flag, if any (`None` when browse overlay, no player, or unknown duration).
fn mpv_pause_state(ctx: &PlayToggleCtx) -> Option<bool> {
    if ctx.recent.is_visible() {
        return None;
    }
    let g = ctx.player.borrow();
    let b = g.as_ref()?;
    if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
        return None;
    }
    Some(b.mpv.get_property::<bool>("pause").unwrap_or(false))
}

/// First enter per fullscreen session: remember pre-entry pause; unpause only when paused before.
fn fs_on_enter_pause(play: &PlayToggleCtx, stash: &RefCell<Option<bool>>) {
    if stash.borrow().is_some() {
        return;
    }
    let Some(was_paused) = mpv_pause_state(play) else {
        return;
    };
    *stash.borrow_mut() = Some(was_paused);
    if was_paused {
        let _ = apply_mpv_pause(play, false);
    }
}

/// Leave fullscreen: re-pause only when entry had unpaused a paused title and playback is still running.
fn fs_on_exit_pause(play: &PlayToggleCtx, stash: &RefCell<Option<bool>>) {
    if stash.borrow_mut().take() != Some(true) {
        return;
    }
    if mpv_pause_state(play) == Some(false) {
        let _ = apply_mpv_pause(play, true);
    }
}

include!("play_toggle.rs");
