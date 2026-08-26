// GL-area pointer motion: theater-mode cursor show/hide with a deferred hide timer.

/// (Re)arm the theater-mode cursor-hide timer used by GL-area motion / enter.
fn arm_theater_cursor_hide(
    cur: &Rc<RefCell<Option<glib::SourceId>>>,
    gl: &gtk::GLArea,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    ptr: &Rc<Cell<bool>>,
) {
    replace_timeout(cur.clone(), {
        let gl2 = gl.clone();
        let win2 = win.clone();
        let player2 = player.clone();
        let ptr2 = ptr.clone();
        move || {
            if ptr2.get() {
                apply_theater_cursor_hide(&win2, &gl2, &player2);
            }
        }
    });
}

/// Widgets and state captured by the GL-area motion / enter / leave handlers.
#[derive(Clone)]
struct GlMotionDeps {
    gl: gtk::GLArea,
    win: adw::ApplicationWindow,
    player: Rc<RefCell<Option<MpvBundle>>>,
    cur: Rc<RefCell<Option<glib::SourceId>>>,
    ptr: Rc<Cell<bool>>,
    squelch: Rc<Cell<Option<Instant>>>,
    last_xy: Rc<Cell<Option<(f64, f64)>>>,
}

impl GlMotionDeps {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            gl: ctx.shell.gl.clone(),
            win: ctx.shell.win.clone(),
            player: ctx.player.clone(),
            cur: ctx.cur_t.clone(),
            ptr: ctx.ptr_in_gl.clone(),
            squelch: ctx.motion_squelch.clone(),
            last_xy: ctx.last_gl_xy.clone(),
        }
    }
}

/// Motion over the video surface: show the pointer, then rearm the hide timer on real movement.
fn on_gl_motion(d: &GlMotionDeps, x: f64, y: f64) {
    d.ptr.set(true);
    if motion_sample_stale(&d.squelch, &d.last_xy, x, y) {
        return;
    }
    d.last_xy.set(Some((x, y)));
    show_chrome_pointer(&d.win, &d.gl);
    arm_theater_cursor_hide(&d.cur, &d.gl, &d.win, &d.player, &d.ptr);
}

/// Entering the video surface shows the pointer and rearms the hide timer.
fn on_gl_enter(d: &GlMotionDeps) {
    d.ptr.set(true);
    if motion_squelched(&d.squelch) {
        return;
    }
    show_chrome_pointer(&d.win, &d.gl);
    arm_theater_cursor_hide(&d.cur, &d.gl, &d.win, &d.player, &d.ptr);
}

/// Leaving the video surface restores the chrome pointer and forgets the last position.
fn on_gl_leave(d: &GlMotionDeps) {
    d.ptr.set(false);
    d.last_xy.set(None);
    // Slot may already be cleared in [`shell::w_in_fullscreen`] before synthesized leave.
    drop_glib_source(d.cur.as_ref());
    show_chrome_pointer(&d.win, &d.gl);
}

fn w_in_gl_motion(ctx: &WindowInputCtx) {
    let d = Rc::new(GlMotionDeps::new(ctx));
    let m = gtk::EventControllerMotion::new();
    let d_motion = Rc::clone(&d);
    m.connect_motion(move |_, x, y| on_gl_motion(&d_motion, x, y));
    let d_enter = Rc::clone(&d);
    m.connect_enter(move |_, _x, _y| on_gl_enter(&d_enter));
    let d_leave = Rc::clone(&d);
    m.connect_leave(move |_| on_gl_leave(&d_leave));
    ctx.shell.gl.add_controller(m);
    #[cfg(target_os = "macos")]
    wire_macos_gl_cursor_while_unfocused(ctx);
}
