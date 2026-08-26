// Maximize-state window wiring: route `maximized` notifications through the demaximize /
// maximize-to-fullscreen rules.

/// `!maximized && !fullscreen`: remember the normal size unless a transition latched the skip.
fn max_mode_remember_normal(
    lu: &Rc<RefCell<(i32, i32)>>,
    skip: &Rc<Cell<bool>>,
    w: &adw::ApplicationWindow,
) {
    if !skip.get() {
        *lu.borrow_mut() = win_normal_size(w);
    }
}

/// `maximized && !fullscreen`: stash the restore size once, then enter fullscreen on an idle.
fn max_mode_enter_fs_from_max(
    w: &adw::ApplicationWindow,
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    skip: &Rc<Cell<bool>>,
) {
    if fr.borrow().is_none() {
        *fr.borrow_mut() = Some(*lu.borrow());
    }
    let w_idle = w.clone();
    let skip_idle = skip.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if skip_idle.get() || !w_idle.is_maximized() || w_idle.is_fullscreen() {
            return;
        }
        #[cfg(target_os = "macos")]
        crate::macos_window::enter_fullscreen_from_maximized(&w_idle);
        #[cfg(not(target_os = "macos"))]
        w_idle.fullscreen();
    });
}

/// Linux only — `!maximized && fullscreen`: user un-maximizes while fullscreen → leave fullscreen.
#[cfg(not(target_os = "macos"))]
fn max_mode_demaximize_from_fs(
    skip: &Rc<Cell<bool>>,
    w: &adw::ApplicationWindow,
    fs_busy: &Rc<Cell<bool>>,
) {
    // macOS: GDK often reports `!maximized && fullscreen` during normal fullscreen entry; treating
    // that as demaximize scheduled `unfullscreen_safe` and canceled fullscreen after our idle
    // deferral fix.
    skip.set(true);
    unfullscreen_safe(w, fs_busy.as_ref());
}

/// Route one maximized-notify sample; the demaximize arm is compiled out on macOS.
#[cfg(not(target_os = "macos"))]
fn max_mode_route(
    w: &adw::ApplicationWindow,
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    skip: &Rc<Cell<bool>>,
    fs_busy: &Rc<Cell<bool>>,
) {
    if !w.is_maximized() {
        if !w.is_fullscreen() {
            max_mode_remember_normal(lu, skip, w);
        } else {
            max_mode_demaximize_from_fs(skip, w, fs_busy);
        }
    } else if !w.is_fullscreen() && !skip.get() {
        max_mode_enter_fs_from_max(w, fr, lu, skip);
    }
}

/// Route one maximized-notify sample on macOS (no demaximize-from-fullscreen arm).
#[cfg(target_os = "macos")]
fn max_mode_route(
    w: &adw::ApplicationWindow,
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    skip: &Rc<Cell<bool>>,
) {
    if !w.is_maximized() && !w.is_fullscreen() {
        max_mode_remember_normal(lu, skip, w);
    } else if w.is_maximized() && !w.is_fullscreen() && !skip.get() {
        max_mode_enter_fs_from_max(w, fr, lu, skip);
    }
}

fn w_in_max_mode(ctx: &WindowInputCtx) {
    let fr = ctx.fs_restore.clone();
    let lu = ctx.last_unmax.clone();
    let skip_fs = ctx.skip_max_to_fs.clone();
    #[cfg(not(target_os = "macos"))]
    let fs_busy = Rc::clone(&ctx.fs_transition_busy);
    let win = ctx.shell.win.clone();
    win.connect_maximized_notify(move |w| {
        #[cfg(not(target_os = "macos"))]
        max_mode_route(w, &fr, &lu, &skip_fs, &fs_busy);
        #[cfg(target_os = "macos")]
        max_mode_route(w, &fr, &lu, &skip_fs);
    });
}
