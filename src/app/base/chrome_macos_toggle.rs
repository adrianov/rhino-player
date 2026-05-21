// macOS fullscreen toggle: native state drives exit/enter; defer during AppKit transitions.

#[cfg(target_os = "macos")]
const MACOS_TOGGLE_DEFER: std::time::Duration = std::time::Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_TOGGLE_DEFER_MAX: u8 = 16;

#[cfg(target_os = "macos")]
fn macos_apply_toggle(
    win: &adw::ApplicationWindow,
    fs_restore: Option<&RefCell<Option<(i32, i32)>>>,
    last_unmax: Option<&RefCell<(i32, i32)>>,
    skip_max: Option<&Cell<bool>>,
) {
    if crate::macos_window::clear_stale_gtk_fullscreen(win) {
        return;
    }

    if crate::macos_window::ns_fullscreen_for_win(win) {
        if let Some(skip) = skip_max {
            skip.set(true);
        }
        macos_schedule_unfullscreen(win.clone());
        return;
    }

    if !win.is_maximized() {
        let dims = fs_restore
            .map(|_| win_normal_size(win))
            .or_else(|| last_unmax.map(|lu| *lu.borrow()))
            .unwrap_or_else(|| win_normal_size(win));
        if let Some(fr) = fs_restore {
            *fr.borrow_mut() = Some(dims);
        }
        win.maximize();
        return;
    }

    if let (Some(fr), Some(lu)) = (fs_restore, last_unmax) {
        if fr.borrow().is_none() {
            *fr.borrow_mut() = Some(*lu.borrow());
        }
    }
    crate::macos_fs_debug::log("enter fullscreen");
    crate::macos_window::enter_fullscreen_from_maximized(win);
}

#[cfg(target_os = "macos")]
fn macos_defer_toggle(win: adw::ApplicationWindow, retry: u8) {
    let win2 = win.clone();
    let _ = glib::timeout_add_local_once(MACOS_TOGGLE_DEFER, move || {
        let gtk = win2.upcast_ref::<gtk::Widget>();
        if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk)
            && retry < MACOS_TOGGLE_DEFER_MAX
        {
            macos_defer_toggle(win2, retry.saturating_add(1));
            return;
        }
        macos_apply_toggle(&win2, None, None, None);
    });
}

/// macOS: do not use [`fs_transition_try_begin`] — it blocks rapid Enter after exit for ~380 ms.
#[cfg(target_os = "macos")]
pub(super) fn macos_toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
) {
    let gtk = win.upcast_ref::<gtk::Widget>();
    if crate::macos_window::gdk_macos_in_fullscreen_transition(gtk) {
        macos_defer_toggle(win.clone(), 0);
        return;
    }
    macos_apply_toggle(win, Some(fs_restore), Some(last_unmax), Some(skip_max_to_fs));
}
