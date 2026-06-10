/// Modifier mask: digit speed shortcuts ignored when Ctrl / Alt / Meta / Super are held.
const DIGIT_SPEED_BLOCK: gtk::gdk::ModifierType = gtk::gdk::ModifierType::CONTROL_MASK
    .union(gtk::gdk::ModifierType::ALT_MASK)
    .union(gtk::gdk::ModifierType::META_MASK)
    .union(gtk::gdk::ModifierType::SUPER_MASK);

#[derive(Clone)]
struct DigitSpeedShortcutCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    play_toggle: PlayToggleCtx,
    gl: gtk::GLArea,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    speed_sync: Rc<Cell<bool>>,
    speed_menu: gtk::MenuButton,
    speed_list: gtk::ListBox,
    speed_readout: gtk::Label,
}

fn digit_speed_multiplier(key: gtk::gdk::Key) -> Option<u8> {
    match key {
        gtk::gdk::Key::_1 | gtk::gdk::Key::KP_1 => Some(1),
        gtk::gdk::Key::_2 | gtk::gdk::Key::KP_2 => Some(2),
        gtk::gdk::Key::_3 | gtk::gdk::Key::KP_3 => Some(3),
        gtk::gdk::Key::_4 | gtk::gdk::Key::KP_4 => Some(4),
        gtk::gdk::Key::_5 | gtk::gdk::Key::KP_5 => Some(5),
        gtk::gdk::Key::_6 | gtk::gdk::Key::KP_6 => Some(6),
        gtk::gdk::Key::_7 | gtk::gdk::Key::KP_7 => Some(7),
        gtk::gdk::Key::_8 | gtk::gdk::Key::KP_8 => Some(8),
        _ => None,
    }
}

/// Digit 3 maps to 1.5×; digits 1, 2, and 4–8 map to N×.
fn digit_speed_value(n: u8) -> f64 {
    if n == 3 { 1.5 } else { f64::from(n) }
}

fn schedule_digit_speed_resync(c: DigitSpeedShortcutCtx, v: f64) {
    let DigitSpeedShortcutCtx {
        player,
        video_pref,
        app,
        speed_sync,
        speed_menu,
        speed_list,
        speed_readout,
        ..
    } = c;
    let bref = player.clone();
    let vp2 = Rc::clone(&video_pref);
    let ap2 = app.clone();
    let sy = speed_sync.clone();
    let spd_m = speed_menu.clone();
    let sl = speed_list.clone();
    let spd_lbl = speed_readout.clone();
    let _ = glib::idle_add_local_once(move || {
        if bref.borrow().is_none() {
            return;
        }
        let r = video_pref::refresh_smooth_for_playback_speed(&bref, &mut vp2.borrow_mut(), Some(v));
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&ap2);
        }
        if let Some(pl) = bref.borrow().as_ref() {
            let _ = playback_speed::sync_list(&pl.mpv, &sy, &sl, &spd_m, &spd_lbl);
        }
    });
}

fn try_digit_speed_shortcut(
    key: gtk::gdk::Key,
    m: gtk::gdk::ModifierType,
    c: &DigitSpeedShortcutCtx,
) -> Option<glib::Propagation> {
    let n = digit_speed_multiplier(key)?;
    if m.intersects(DIGIT_SPEED_BLOCK) {
        return Some(glib::Propagation::Proceed);
    }
    let v = digit_speed_value(n);
    crate::user_action_log::act(format!("key digit {n} -> speed {v:.1}×"));
    let g = c.player.borrow();
    let Some(b) = g.as_ref() else {
        return Some(glib::Propagation::Proceed);
    };
    let need_unpause = b.mpv.get_property::<bool>("pause").unwrap_or(false);
    drop(g);
    if need_unpause {
        let _ = apply_mpv_pause(&c.play_toggle, false);
    }
    let g = c.player.borrow();
    let Some(b) = g.as_ref() else {
        return Some(glib::Propagation::Proceed);
    };
    if b.mpv.set_property("speed", v).is_err() {
        return Some(glib::Propagation::Proceed);
    }
    drop(g);
    playback_speed::stamp_header(&c.speed_menu, &c.speed_readout, v);
    c.gl.queue_render();
    schedule_digit_speed_resync(c.clone(), v);
    Some(glib::Propagation::Stop)
}
