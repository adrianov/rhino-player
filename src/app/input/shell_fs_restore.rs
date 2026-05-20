fn restore_windowed_size(
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    lu: &Rc<RefCell<(i32, i32)>>,
    w: &adw::ApplicationWindow,
) {
    use gtk::prelude::NativeExt;

    let dims = fr.borrow_mut().take().or(Some(*lu.borrow()));
    let Some((gw, gh)) = dims else { return };
    // Clear maximized state explicitly: after fullscreen the compositor may not match `is_maximized`.
    w.set_default_size(gw, gh);
    w.set_maximized(false);
    w.unmaximize();
    w.set_default_size(gw, gh);
    if let Some(surface) = w.surface() {
        surface.request_layout();
    }
    w.present();
}

#[cfg(not(target_os = "macos"))]
fn schedule_leave_fs_idle_linux(
    fr_leave: Rc<RefCell<Option<(i32, i32)>>>,
    lu_leave: Rc<RefCell<(i32, i32)>>,
    w_leave: adw::ApplicationWindow,
    skip_leave: Rc<Cell<bool>>,
    tch_leave: Rc<dyn Fn(&adw::ApplicationWindow)>,
    play_leave: PlayToggleCtx,
    pause_leave: Rc<RefCell<Option<bool>>>,
) {
    let _ = glib::source::idle_add_local_once(move || {
        fs_on_exit_pause(&play_leave, pause_leave.as_ref());
        restore_windowed_size(&fr_leave, &lu_leave, &w_leave);
        let w2 = w_leave;
        let skip2 = skip_leave;
        let tch2 = tch_leave;
        glib::source::idle_add_local_once(move || {
            skip2.set(false);
            tch2(&w2);
        });
    });
}

#[cfg(target_os = "macos")]
fn macos_schedule_leave_fs_restore_chrome(
    gen: &Rc<Cell<u32>>,
    delay: std::time::Duration,
    want_gen: u32,
    fr_leave: Rc<RefCell<Option<(i32, i32)>>>,
    lu_leave: Rc<RefCell<(i32, i32)>>,
    w_leave: adw::ApplicationWindow,
    skip_leave: Rc<Cell<bool>>,
    tch_leave: Rc<dyn Fn(&adw::ApplicationWindow)>,
    play_leave: PlayToggleCtx,
    pause_leave: Rc<RefCell<Option<bool>>>,
) {
    let gen_rc = Rc::clone(gen);
    let _ = glib::timeout_add_local_once(delay, move || {
        if gen_rc.get() != want_gen || w_leave.is_fullscreen() {
            skip_leave.set(false);
            return;
        }
        fs_on_exit_pause(&play_leave, pause_leave.as_ref());
        restore_windowed_size(&fr_leave, &lu_leave, &w_leave);
        let w2 = w_leave;
        let skip2 = skip_leave;
        let tch2 = tch_leave;
        glib::source::idle_add_local_once(move || {
            skip2.set(false);
            tch2(&w2);
        });
    });
}
