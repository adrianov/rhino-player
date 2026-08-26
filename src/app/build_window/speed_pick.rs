fn dismiss_speed_menu(btn: &gtk::MenuButton) {
    #[cfg(target_os = "macos")]
    {
        btn.remove_css_class("rp-header-menu-open");
        crate::macos_header_menu_overlay::overlay_close_all("speed_pick");
    }
    if let Some(pop) = btn.popover() {
        pop.popdown();
    }
    btn.set_active(false);
}

fn speed_row_index(list: &gtk::ListBox, row: &gtk::ListBoxRow) -> u32 {
    (0i32..playback_speed::SPEEDS.len() as i32)
        .find(|&ix| list.row_at_index(ix).is_some_and(|r| r == *row))
        .unwrap_or(0) as u32
}

/// Everything a speed-row pick needs; cloned once into the row signal handler.
#[derive(Clone)]
struct SpeedPick {
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    mbtn: gtk::MenuButton,
    readout: gtk::Label,
    sync: Rc<Cell<bool>>,
    pick: Rc<Cell<bool>>,
}

fn apply_speed_row_pick(c: &SpeedPick, list: &gtk::ListBox, row: &gtk::ListBoxRow) {
    if speed_pick_skipped(c, list) {
        return;
    }
    let v = playback_speed::value_at(speed_row_index(list, row));
    crate::user_action_log::act(format!("speed menu row -> {v:.1}×"));
    #[cfg(target_os = "macos")]
    crate::macos_header_menu_debug::log_event("speed", "row_apply", &format!("rate={v}"));
    if set_mpv_speed(c, v) {
        playback_speed::stamp_header(&c.mbtn, &c.readout, v);
        dismiss_speed_menu(&c.mbtn);
        c.gl.queue_render();
        queue_smooth_refresh_after_speed(c, v);
    }
}

/// Guard: ignore picks during programmatic sync, the opening click settle, or insensitive list.
fn speed_pick_skipped(c: &SpeedPick, list: &gtk::ListBox) -> bool {
    let skipped = c.sync.get() || c.pick.get() || !list.is_sensitive();
    #[cfg(target_os = "macos")]
    if skipped {
        crate::macos_header_menu_debug::log_event(
            "speed",
            "row_pick_skip",
            &format!(
                "sync={} pick={} sensitive={}",
                c.sync.get(),
                c.pick.get(),
                list.is_sensitive()
            ),
        );
    }
    skipped
}

/// Writes the rate to mpv; `false` when no player bundle exists or the write fails.
fn set_mpv_speed(c: &SpeedPick, v: f64) -> bool {
    let guard = c.player.borrow();
    let Some(b) = guard.as_ref() else {
        eprintln!("[rhino] speed: row pick with no player bundle");
        return false;
    };
    if b.mpv.set_property("speed", v).is_err() {
        eprintln!("[rhino] speed: set_property speed={v} failed");
        return false;
    }
    true
}

fn queue_smooth_refresh_after_speed(c: &SpeedPick, v: f64) {
    let player_idle = Rc::clone(&c.player);
    let vp_idle = Rc::clone(&c.video_pref);
    let app_idle = c.app.clone();
    let _ = glib::idle_add_local_once(move || {
        if player_idle.borrow().as_ref().is_none() {
            return;
        }
        let r = video_pref::refresh_smooth_for_playback_speed(
            &player_idle,
            &mut vp_idle.borrow_mut(),
            Some(v),
        );
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&app_idle);
        }
    });
}
