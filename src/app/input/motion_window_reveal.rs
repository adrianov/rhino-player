// Window-level capture-phase pointer motion: reveal chrome bars on real movement.

/// Widgets and state captured by the window capture-phase motion handler.
#[derive(Clone)]
struct WinMotionDeps {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    hdr_csd: Rc<Cell<Option<(bool, bool)>>>,
    header: adw::HeaderBar,
    gl: gtk::GLArea,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    bars_shown: Rc<Cell<bool>>,
    ch_hide: Rc<ChromeBarHide>,
    squelch: Rc<Cell<Option<Instant>>>,
    last_xy: Rc<Cell<Option<(f64, f64)>>>,
}

impl WinMotionDeps {
    fn new(ctx: &WindowInputCtx) -> Self {
        Self {
            win: ctx.shell.win.clone(),
            root: ctx.shell.root.clone(),
            hdr_csd: Rc::clone(&ctx.hdr_csd_baseline),
            header: ctx.shell.header.clone(),
            gl: ctx.shell.gl.clone(),
            recent: ctx.shell.recent.clone(),
            bottom: ctx.shell.bottom.clone(),
            player: ctx.player.clone(),
            bars_shown: ctx.bar_show.clone(),
            ch_hide: Rc::clone(&ctx.ch_hide),
            squelch: ctx.motion_squelch.clone(),
            last_xy: ctx.last_cap_xy.clone(),
        }
    }
}

/// One capture-phase motion sample over the window: dedupe, show the pointer, reveal bars once,
/// rearm auto-hide.
fn on_window_motion(d: &WinMotionDeps, x: f64, y: f64) {
    if d.recent.is_visible() {
        return;
    }
    if motion_sample_stale(&d.squelch, &d.last_xy, x, y) {
        return;
    }
    d.last_xy.set(Some((x, y)));
    show_chrome_pointer(&d.win, &d.gl);
    reveal_bars_once(
        &d.bars_shown,
        ChromeApplyParts {
            hdr_csd_baseline: &d.hdr_csd,
            root: &d.root,
            header: &d.header,
            gl: &d.gl,
            bar_show: &d.bars_shown,
            recent: &d.recent,
            bottom: &d.bottom,
            player: &d.player,
        },
    );
    schedule_bars_autohide(Rc::clone(&d.ch_hide));
}

fn w_in_win_motion(ctx: &WindowInputCtx) {
    let cap = gtk::EventControllerMotion::new();
    cap.set_propagation_phase(gtk::PropagationPhase::Capture);
    let d = WinMotionDeps::new(ctx);
    cap.connect_motion(move |_, x, y| on_window_motion(&d, x, y));
    ctx.shell.win.add_controller(cap);
}
