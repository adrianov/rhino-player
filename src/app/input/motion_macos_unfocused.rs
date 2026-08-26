include!("motion_macos_unfocused_tick.rs");

/// While the window is not key, GTK does not deliver [`EventControllerMotion`] on the [`GLArea`],
/// so the normal idle cursor hide never runs. An [`NSEvent`] global mouse-moved monitor (fires
/// while this app is inactive) mirrors the focused hide path when the pointer is over our
/// frontmost video surface.
fn wire_macos_gl_cursor_while_unfocused(ctx: &WindowInputCtx) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;

    let cursor = UnfocusedCursor::from_ctx(ctx);
    let monitor = Rc::new(RefCell::new(None::<Retained<AnyObject>>));
    let ctl = make_unfocused_monitor_ctl(&monitor, &cursor);

    wire_unfocused_active_watch(&cursor, &ctl.start, &ctl.stop);
    wire_unfocused_gl_map_watch(&cursor, &ctl.start);
    wire_unfocused_recent_watch(&cursor, &ctl.start, &ctl.stop);
    watch_window_occlusion(cursor.win.clone(), cursor.clone());

    if !cursor.win.is_active() {
        (ctl.start)();
        cursor.hide_now_if_over_video();
    }
}

impl UnfocusedCursor {
    fn from_ctx(ctx: &WindowInputCtx) -> Self {
        Self {
            win: ctx.shell.win.clone(),
            gl: ctx.shell.gl.clone(),
            recent: ctx.shell.recent.clone(),
            player: ctx.player.clone(),
            cur: ctx.cur_t.clone(),
            ptr: ctx.ptr_in_gl.clone(),
            sq: ctx.motion_squelch.clone(),
            lgl: ctx.last_gl_xy.clone(),
        }
    }
}

/// Start/stop controls for the global mouse-moved monitor plus its tick closure.
struct UnfocusedMonitorCtl {
    start: Rc<dyn Fn()>,
    stop: Rc<dyn Fn()>,
}

fn make_unfocused_monitor_ctl(
    monitor: &Rc<MouseMon>,
    cursor: &UnfocusedCursor,
) -> UnfocusedMonitorCtl {
    let tick: Rc<dyn Fn()> = Rc::new({
        let cursor = cursor.clone();
        move || cursor.tick()
    });
    let start_monitor: Rc<dyn Fn()> = {
        let monitor = Rc::clone(monitor);
        let tick = Rc::clone(&tick);
        Rc::new(move || start_unfocused_mouse_monitor(&monitor, &tick))
    };
    let stop_monitor: Rc<dyn Fn()> = {
        let monitor = Rc::clone(monitor);
        Rc::new(move || drop_mouse_monitor(&monitor))
    };
    UnfocusedMonitorCtl {
        start: start_monitor,
        stop: stop_monitor,
    }
}

/// Window key-state changes: monitor only while inactive; on activation restore the pointer.
fn wire_unfocused_active_watch(
    cursor: &UnfocusedCursor,
    start_monitor: &Rc<dyn Fn()>,
    stop_monitor: &Rc<dyn Fn()>,
) {
    let c_act = cursor.clone();
    let stop_act = Rc::clone(stop_monitor);
    let start_act = Rc::clone(start_monitor);
    cursor.win.connect_is_active_notify(move |w| {
        if w.is_active() {
            stop_act();
            if !pointer_over_video_gl(w, &c_act.gl) {
                c_act.leave();
            }
            return;
        }
        start_act();
        c_act.hide_now_if_over_video();
    });
}

/// GL area mapped while the window is inactive: start hiding immediately.
fn wire_unfocused_gl_map_watch(cursor: &UnfocusedCursor, start_monitor: &Rc<dyn Fn()>) {
    let win_map = cursor.win.clone();
    let c_map = cursor.clone();
    let start_map = Rc::clone(start_monitor);
    cursor.gl.connect_map(move |_| {
        if win_map.is_active() {
            return;
        }
        start_map();
        c_map.hide_now_if_over_video();
    });
}

/// Recent grid visibility while the window is inactive toggles the hide monitor.
fn wire_unfocused_recent_watch(
    cursor: &UnfocusedCursor,
    start_monitor: &Rc<dyn Fn()>,
    stop_monitor: &Rc<dyn Fn()>,
) {
    let c_rv = cursor.clone();
    let stop_rv = Rc::clone(stop_monitor);
    let start_rv = Rc::clone(start_monitor);
    cursor.recent.connect_visible_notify(move |r| {
        if c_rv.win.is_active() {
            return;
        }
        if r.is_visible() {
            stop_rv();
            c_rv.leave();
            return;
        }
        if c_rv.theater_ready() {
            start_rv();
            c_rv.hide_now_if_over_video();
        }
    });
}
