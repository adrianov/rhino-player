/// While the window is not key, GTK does not deliver [`EventControllerMotion`] on the [`GLArea`],
/// so the normal 3s cursor hide never runs. Poll pointer position via GDK and mirror the same hide
/// path when the device is over our surface and [`gtk::Widget::pick`] hits the video [`GLArea`].
fn wire_macos_gl_cursor_while_unfocused(ctx: &WindowInputCtx) {
    use glib::prelude::Cast;
    use gtk::prelude::WidgetExt;

    const POLL_MS: u64 = 200;
    const MEDIA_RETRY_S: u32 = 3;

    fn ptr_leave_gl(
        cur: &Rc<RefCell<Option<glib::SourceId>>>,
        ptr: &Cell<bool>,
        lgl: &Cell<Option<(f64, f64)>>,
        win: &adw::ApplicationWindow,
        gl: &gtk::GLArea,
    ) {
        ptr.set(false);
        lgl.set(None);
        if let Some(id) = cur.borrow_mut().take() {
            id.remove();
        }
        show_chrome_pointer(win, gl);
    }

    fn gl_arm_hide_timer(
        cur: &Rc<RefCell<Option<glib::SourceId>>>,
        win: &adw::ApplicationWindow,
        gl: &gtk::GLArea,
        player: &Rc<RefCell<Option<MpvBundle>>>,
        ptr: &Rc<Cell<bool>>,
    ) {
        let win2 = win.clone();
        let gl2 = gl.clone();
        let player2 = Rc::clone(player);
        let ptr2 = ptr.clone();
        replace_timeout(Rc::clone(cur), move || {
            hide_gl_cursor_after_idle(&win2, &gl2, &player2, &ptr2);
        });
    }

    fn hide_gl_cursor_after_idle(
        win: &adw::ApplicationWindow,
        gl: &gtk::GLArea,
        player: &Rc<RefCell<Option<MpvBundle>>>,
        ptr: &Cell<bool>,
    ) {
        if !ptr.get() {
            return;
        }
        let Some((px, py)) = pointer_pick_xy_for_win(win) else {
            ptr.set(false);
            show_chrome_pointer(win, gl);
            return;
        };
        if !pointer_in_window_client(win, px, py) {
            ptr.set(false);
            show_chrome_pointer(win, gl);
            return;
        }
        let gl_w: gtk::Widget = gl.clone().upcast();
        let over_gl = win
            .pick(px, py, gtk::PickFlags::DEFAULT)
            .is_some_and(|p| p == gl_w);
        if !over_gl {
            ptr.set(false);
            show_chrome_pointer(win, gl);
            return;
        }
        if !apply_theater_cursor_hide(win, gl, player) {
            ptr.set(false);
            show_chrome_pointer(win, gl);
        }
    }

    fn cancel_wake(media_wake: &Rc<RefCell<Option<glib::SourceId>>>) {
        if let Some(id) = media_wake.borrow_mut().take() {
            id.remove();
        }
    }

    // After we stop the 200 ms poll (no playable media), wake every few seconds to see if we
    // should poll again — avoids running the tight timer when the window sits unfocused for hours.
    fn arm_media_poll_wake(
        media_wake: &Rc<RefCell<Option<glib::SourceId>>>,
        win: adw::ApplicationWindow,
        gl: gtk::GLArea,
        recent: gtk::Box,
        player: Rc<RefCell<Option<MpvBundle>>>,
        start_poll: Rc<dyn Fn()>,
    ) {
        cancel_wake(media_wake);
        let mw = Rc::clone(media_wake);
        let sp = Rc::clone(&start_poll);
        *media_wake.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_secs(u64::from(MEDIA_RETRY_S)),
            move || {
                cancel_wake(&mw);
                if win.is_active() || !gl.is_mapped() {
                    return;
                }
                if recent.is_visible() || !chrome_should_hide_cursor_for_media(&player) {
                    arm_media_poll_wake(&mw, win.clone(), gl.clone(), recent.clone(), Rc::clone(&player), Rc::clone(&sp));
                    return;
                }
                sp();
            },
        ));
    }

    let win = ctx.shell.win.clone();
    let gl = ctx.shell.gl.clone();
    let recent = ctx.shell.recent.clone();
    let player = ctx.player.clone();
    let cur = ctx.cur_t.clone();
    let ptr = ctx.ptr_in_gl.clone();
    let sq = ctx.motion_squelch.clone();
    let lgl = ctx.last_gl_xy.clone();
    let poll_slot = Rc::new(RefCell::new(None::<glib::SourceId>));
    let media_wake = Rc::new(RefCell::new(None::<glib::SourceId>));

    let stop_poll_timer: Rc<dyn Fn()> = {
        let poll_slot = Rc::clone(&poll_slot);
        Rc::new(move || {
            if let Some(id) = poll_slot.borrow_mut().take() {
                id.remove();
            }
        })
    };

    let cancel_wake_f: Rc<dyn Fn()> = {
        let media_wake = Rc::clone(&media_wake);
        Rc::new(move || cancel_wake(&media_wake))
    };

    type StartPollCell = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

    let start_poll_slot: StartPollCell = Rc::new(RefCell::new(None));

    let tick: Rc<dyn Fn() -> glib::ControlFlow> = Rc::new({
        let win = win.clone();
        let gl = gl.clone();
        let recent = recent.clone();
        let player = player.clone();
        let cur = cur.clone();
        let ptr = ptr.clone();
        let sq = sq.clone();
        let lgl = lgl.clone();
        let stop_poll_timer = Rc::clone(&stop_poll_timer);
        let media_wake = Rc::clone(&media_wake);
        let start_poll_slot = Rc::clone(&start_poll_slot);
        let cancel_wake_tick = Rc::clone(&cancel_wake_f);
        move || {
            if win.is_active() {
                return glib::ControlFlow::Break;
            }
            if !gl.is_mapped() || !gl.is_visible() {
                cancel_wake_tick();
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                stop_poll_timer();
                return glib::ControlFlow::Break;
            }
            if recent.is_visible() {
                cancel_wake_tick();
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                stop_poll_timer();
                return glib::ControlFlow::Break;
            }
            if !chrome_should_hide_cursor_for_media(&player) {
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                stop_poll_timer();
                let Some(start_poll) = start_poll_slot.borrow().clone() else {
                    return glib::ControlFlow::Break;
                };
                arm_media_poll_wake(
                    &media_wake,
                    win.clone(),
                    gl.clone(),
                    recent.clone(),
                    Rc::clone(&player),
                    start_poll,
                );
                return glib::ControlFlow::Break;
            }
            let Some((x, y)) = pointer_pick_xy_for_win(&win) else {
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                return glib::ControlFlow::Continue;
            };
            if !pointer_in_window_client(&win, x, y) {
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                return glib::ControlFlow::Continue;
            }
            let gl_w: gtk::Widget = gl.clone().upcast();
            let over_gl = win.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|p| p == gl_w);
            if !over_gl {
                ptr_leave_gl(&cur, &ptr, &lgl, &win, &gl);
                return glib::ControlFlow::Continue;
            }
            ptr.set(true);
            if let Some(t) = sq.get() {
                if Instant::now() < t {
                    return glib::ControlFlow::Continue;
                }
            }
            if let Some((lx, ly)) = lgl.get() {
                if same_xy(x, lx) && same_xy(y, ly) {
                    return glib::ControlFlow::Continue;
                }
            }
            lgl.set(Some((x, y)));
            show_chrome_pointer(&win, &gl);
            gl_arm_hide_timer(&cur, &win, &gl, &player, &ptr);
            glib::ControlFlow::Continue
        }
    });

    let start_poll: Rc<dyn Fn()> = {
        let poll_slot = Rc::clone(&poll_slot);
        let tick = Rc::clone(&tick);
        let cancel_wake_f = Rc::clone(&cancel_wake_f);
        Rc::new(move || {
            cancel_wake_f();
            if poll_slot.borrow().is_some() {
                return;
            }
            let tick2 = Rc::clone(&tick);
            *poll_slot.borrow_mut() = Some(glib::source::timeout_add_local(
                Duration::from_millis(POLL_MS),
                move || tick2(),
            ));
        })
    };
    *start_poll_slot.borrow_mut() = Some(start_poll.clone());

    let stop_all: Rc<dyn Fn()> = {
        let cancel_wake_f = Rc::clone(&cancel_wake_f);
        let stop_poll_timer = Rc::clone(&stop_poll_timer);
        Rc::new(move || {
            cancel_wake_f();
            stop_poll_timer();
        })
    };

    let gl_act = gl.clone();
    let cur_act = cur.clone();
    let ptr_act = ptr.clone();
    let lgl_act = lgl.clone();
    let stop_all_act = Rc::clone(&stop_all);
    let start_act = Rc::clone(&start_poll);
    let player_vis = Rc::clone(&player);
    win.connect_is_active_notify(move |w| {
        if w.is_active() {
            stop_all_act();
            ptr_leave_gl(&cur_act, &ptr_act, &lgl_act, w, &gl_act);
        } else {
            start_act();
        }
    });

    let win_map = win.clone();
    let start_map = Rc::clone(&start_poll);
    gl.connect_map(move |_| {
        if win_map.is_active() {
            return;
        }
        start_map();
    });

    let win_rv = win.clone();
    let gl_rv = gl.clone();
    let stop_rv = Rc::clone(&stop_all);
    let start_rv = Rc::clone(&start_poll);
    let cur_rv = cur.clone();
    let ptr_rv = ptr.clone();
    let lgl_rv = lgl.clone();
    recent.connect_visible_notify(move |r| {
        if win_rv.is_active() {
            return;
        }
        if r.is_visible() {
            stop_rv();
            ptr_leave_gl(&cur_rv, &ptr_rv, &lgl_rv, &win_rv, &gl_rv);
            return;
        }
        if gl_rv.is_mapped()
            && gl_rv.is_visible()
            && chrome_should_hide_cursor_for_media(&player_vis)
        {
            start_rv();
        }
    });

    if !win.is_active() {
        start_poll();
    }
}
