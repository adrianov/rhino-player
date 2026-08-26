// Subtitle header-button visibility: checked on the load drain (the track list is normally
// settled by the time playback opens), revealing the button on first hit and re-applying the
// saved / auto-picked track once. A bounded poll remains only as a last-resort fallback.
//
// Included from `chrome_fullscreen_and_fit.rs`; shares its module scope.

/// Whether any subtitle track exists on the current mpv playback.
fn subs_present(b: &MpvBundle) -> bool {
    let shell = b.me_budget_shell_path.borrow();
    sub_tracks::has_subtitle_tracks(&b.mpv, shell.as_deref())
}

/// Re-apply the saved subtitle preference or auto-pick a track once.
fn reapply_or_autopick_subs(b: &MpvBundle) {
    let pr = crate::db::load_sub();
    let shell = b.me_budget_shell_path.borrow();
    sub_tracks::reapply_saved_or_autopick(&b.mpv, &pr, shell.as_deref());
}

/// One fallback poll: show the button when tracks appear; returns whether the scan settled.
fn sub_scan_tick(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    button: &gtk::MenuButton,
    tries: &Rc<Cell<u8>>,
) -> bool {
    let has_subs = player.borrow().as_ref().is_some_and(subs_present);
    button.set_visible(has_subs);
    if has_subs {
        if let Some(b) = player.borrow().as_ref() {
            reapply_or_autopick_subs(b);
        }
        return true;
    }
    let next = tries.get().saturating_add(1);
    tries.set(next);
    next >= SUB_SCAN_TICKS
}

/// Last-resort bounded poll for tracks that appear after the load drain: mpv exposes no
/// "track-list settled" signal, so their *absence* cannot be observed as an event; give up
/// after [`SUB_SCAN_TICKS`] empty ticks instead of polling forever.
fn schedule_sub_button_poll(player: Rc<RefCell<Option<MpvBundle>>>, button: gtk::MenuButton) {
    let tries = Rc::new(Cell::new(0u8));
    let _ = glib::timeout_add_local(Duration::from_millis(SUB_SCAN_MS), move || {
        if sub_scan_tick(&player, &button, &tries) {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

/// Load-drain entry: check the track list once right now and arm the fallback poll only
/// while tracks are still missing.
fn sync_sub_button_after_load(player: Rc<RefCell<Option<MpvBundle>>>, button: gtk::MenuButton) {
    button.set_visible(false);
    let found = player.borrow().as_ref().is_some_and(subs_present);
    button.set_visible(found);
    if !found {
        schedule_sub_button_poll(player, button);
        return;
    }
    if let Some(b) = player.borrow().as_ref() {
        reapply_or_autopick_subs(b);
    }
}
