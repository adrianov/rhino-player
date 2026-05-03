fn propagation_escape_key(
    key: gtk::gdk::Key,
    win: &adw::ApplicationWindow,
    skip_max_to_fs: &Rc<Cell<bool>>,
    fs_transition_busy: &Rc<Cell<bool>>,
    recent: &gtk::Box,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    browse_back: &Rc<dyn Fn(bool)>,
) -> Option<glib::Propagation> {
    if key != gtk::gdk::Key::Escape {
        return None;
    }
    if win.is_fullscreen() {
        if !fs_transition_try_begin(fs_transition_busy.as_ref()) {
            return Some(glib::Propagation::Stop);
        }
        skip_max_to_fs.set(true);
        unfullscreen_safe_inner(win);
        return Some(glib::Propagation::Stop);
    }
    if recent.is_visible() {
        return Some(glib::Propagation::Stop);
    }
    if player.borrow().is_none() {
        return Some(glib::Propagation::Stop);
    }
    browse_back(true);
    Some(glib::Propagation::Stop)
}

fn propagation_horizontal_seek(
    key: gtk::gdk::Key,
    grid_visible: bool,
    seek_sensitive: bool,
    deps: &SeekArrowDeps<'_>,
) -> Option<glib::Propagation> {
    let delta = match key {
        gtk::gdk::Key::Left | gtk::gdk::Key::KP_Left => -5.0,
        gtk::gdk::Key::Right | gtk::gdk::Key::KP_Right => 5.0,
        _ => return None,
    };
    if grid_visible || !seek_sensitive {
        return Some(glib::Propagation::Proceed);
    }
    seek_arrow_step(deps, delta);
    Some(glib::Propagation::Stop)
}
