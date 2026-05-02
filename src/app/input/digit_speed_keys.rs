/// Modifier mask: digit speed shortcuts ignored when Ctrl / Alt / Meta / Super are held.
const DIGIT_SPEED_BLOCK: gtk::gdk::ModifierType = gtk::gdk::ModifierType::CONTROL_MASK
    .union(gtk::gdk::ModifierType::ALT_MASK)
    .union(gtk::gdk::ModifierType::META_MASK)
    .union(gtk::gdk::ModifierType::SUPER_MASK);

#[derive(Clone)]
struct DigitSpeedShortcutCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    gl: gtk::GLArea,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
    speed_sync: Rc<Cell<bool>>,
    speed_list: gtk::ListBox,
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

fn schedule_digit_speed_resync(c: DigitSpeedShortcutCtx, v: f64) {
    let DigitSpeedShortcutCtx { player, video_pref, app, speed_sync, speed_list, .. } = c;
    let bref = player.clone();
    let vp2 = Rc::clone(&video_pref);
    let ap2 = app.clone();
    let sy = speed_sync.clone();
    let sl = speed_list.clone();
    let _ = glib::idle_add_local_once(move || {
        let Some(ref pl) = *bref.borrow() else { return };
        let r = video_pref::refresh_smooth_for_playback_speed(pl, &mut vp2.borrow_mut(), Some(v));
        if r.smooth_auto_off {
            sync_smooth_60_to_off(&ap2);
        }
        let _ = playback_speed::sync_list(&pl.mpv, &sy, &sl);
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
    let v = f64::from(n);
    let g = c.player.borrow();
    let Some(b) = g.as_ref() else {
        return Some(glib::Propagation::Proceed);
    };
    if b.mpv.set_property("speed", v).is_err() {
        return Some(glib::Propagation::Proceed);
    }
    drop(g);
    c.gl.queue_render();
    schedule_digit_speed_resync(c.clone(), v);
    Some(glib::Propagation::Stop)
}
