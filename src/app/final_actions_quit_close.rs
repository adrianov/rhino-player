fn wire_quit_close(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub_pref: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<crate::idle_inhibit::Held>>>,
    mpv_teardown_after_draw: &Rc<Cell<bool>>,
) {
    let quit = gio::SimpleAction::new("quit", None);
    let p_quit = player.clone();
    let win_q = win.clone();
    let gl_q = gl.clone();
    let sp_quit = sub_pref.clone();
    let idle_q = Rc::clone(idle_inhib);
    let td_quit = Rc::clone(mpv_teardown_after_draw);
    quit.connect_activate(glib::clone!(
        #[strong]
        app,
        #[strong]
        p_quit,
        #[strong]
        win_q,
        #[strong]
        gl_q,
        #[strong]
        sp_quit,
        #[strong]
        idle_q,
        #[strong]
        td_quit,
        move |_, _| {
            schedule_quit_persist(
                &app,
                &win_q,
                &gl_q,
                &p_quit,
                &sp_quit,
                &idle_q,
                &td_quit,
            );
        }
    ));
    app.add_action(&quit);

    let p = player.clone();
    let w = win.clone();
    let gl_close = gl.clone();
    let sp_close = sub_pref.clone();
    let iclose = Rc::clone(idle_inhib);
    let td_close = Rc::clone(mpv_teardown_after_draw);
    win.connect_close_request(glib::clone!(
        #[strong]
        app,
        #[strong]
        p,
        #[strong]
        w,
        #[strong]
        gl_close,
        #[strong]
        sp_close,
        #[strong]
        iclose,
        #[strong]
        td_close,
        move |_win| {
            schedule_quit_persist(
                &app,
                &w,
                &gl_close,
                &p,
                &sp_close,
                &iclose,
                &td_close,
            );
            glib::Propagation::Stop
        }
    ));
}
