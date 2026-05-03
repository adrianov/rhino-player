/// Double-click on the video surface (**GLArea**): skip while browse overlay hides video.
fn wire_gl_double_click_fullscreen(
    gl_area: &gtk::GLArea,
    win: &adw::ApplicationWindow,
    fs_restore: &Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &Rc<Cell<bool>>,
    fs_transition_busy: &Rc<Cell<bool>>,
    recent: &gtk::Box,
) {
    // **connect_pressed** with `n_press == 2` — on some stacks `connect_released` does not report
    // `n_press == 2` reliably.
    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    let (win_fs, fr, lu, skip_dbl, fb_dbl, rec_dbl) = (
        win.clone(),
        Rc::clone(fs_restore),
        Rc::clone(last_unmax),
        Rc::clone(skip_max_to_fs),
        Rc::clone(fs_transition_busy),
        recent.clone(),
    );
    dbl.connect_pressed(move |_, n_press, _, _| {
        if n_press != 2 || rec_dbl.is_visible() {
            return;
        }
        toggle_fullscreen(&win_fs, &fr, &lu, &skip_dbl, fb_dbl.as_ref());
    });
    gl_area.add_controller(dbl);
}

/// Double-click on the GTK header chrome toggles fullscreen the same way as the video surface:
/// fullscreen exit is never blocked when the browse strip is visible; entering fullscreen skips while
/// the strip is visible. On macOS, use [adw::HeaderBar::set_title_widget] so the centered title hits
/// this gesture (native title chrome does not).
fn wire_header_fullscreen_toggle(
    header: &adw::HeaderBar,
    win: &adw::ApplicationWindow,
    fs_restore: &Rc<RefCell<Option<(i32, i32)>>>,
    last_unmax: &Rc<RefCell<(i32, i32)>>,
    skip_max_to_fs: &Rc<Cell<bool>>,
    fs_transition_busy: &Rc<Cell<bool>>,
    recent: &gtk::Box,
) {
    let hdr_dbl = gtk::GestureClick::new();
    hdr_dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    hdr_dbl.set_propagation_phase(gtk::PropagationPhase::Capture);
    hdr_dbl.set_propagation_limit(gtk::PropagationLimit::None);
    let (win_hdr, fr_hdr, lu_hdr, skip_hdr, fb_hdr, rec_hdr) = (
        win.clone(),
        Rc::clone(fs_restore),
        Rc::clone(last_unmax),
        Rc::clone(skip_max_to_fs),
        Rc::clone(fs_transition_busy),
        recent.clone(),
    );
    hdr_dbl.connect_pressed(move |_, n_press, _, _| {
        if n_press != 2 {
            return;
        }
        if rec_hdr.is_visible() && !win_hdr.is_fullscreen() {
            return;
        }
        toggle_fullscreen(&win_hdr, &fr_hdr, &lu_hdr, &skip_hdr, fb_hdr.as_ref());
    });
    header.add_controller(hdr_dbl);
}
