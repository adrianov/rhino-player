// Fullscreen / maximize window-state wiring: the `fullscreened` and `maximized` notify handlers
// plus the chrome-touch factory shared with focus-return repaint.

#[cfg(not(target_os = "macos"))]
fn linux_fs_notify_maximize_now(
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    win: &adw::ApplicationWindow,
) {
    // [`toggle_fullscreen`] already saved geometry before `maximize()`; do not replace it here with
    // sizes read mid-transition (wrong if fullscreen notify races ahead of `is_maximized`).
    if fr.borrow().is_none() {
        *fr.borrow_mut() = Some(win_normal_size(win));
    }
    win.maximize();
}

#[cfg(target_os = "macos")]
fn macos_fs_notify_defer_maximize(
    fr: &Rc<RefCell<Option<(i32, i32)>>>,
    win: &adw::ApplicationWindow,
) {
    let fr_mx = Rc::clone(fr);
    let w_mx = win.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if !w_mx.is_fullscreen() || w_mx.is_maximized() {
            return;
        }
        // Native fullscreen often keeps GDK `is_maximized` false while fullscreen is true, so this
        // path runs after [`toggle_fullscreen`] already stashed pre-maximize (w, h). Replacing
        // `fs_restore` with `win_normal_size` here used the fullscreen-stage dimensions → exit left
        // a maximized / screen-sized window instead of the original floater.
        if fr_mx.borrow().is_none() {
            *fr_mx.borrow_mut() = Some(win_normal_size(&w_mx));
        }
        w_mx.maximize();
    });
}

/// Reapply chrome — shared by fullscreen transitions and focus return.
/// Uses [`apply_chrome_force_layers`] so macOS drops stale layer snapshots even when reveal is unchanged.
fn touch_chrome_gl_factory(ctx: &WindowInputCtx) -> Rc<dyn Fn(&adw::ApplicationWindow)> {
    let root_fs = ctx.shell.root.clone();
    let hdr_csd = Rc::clone(&ctx.hdr_csd_baseline);
    let header_fs = ctx.shell.header.clone();
    let gl_fs = ctx.shell.gl.clone();
    let recent_fs = ctx.shell.recent.clone();
    let bottom_fs = ctx.shell.bottom.clone();
    let p_fs = ctx.player.clone();
    let b = ctx.bar_show.clone();
    Rc::new(move |_w: &adw::ApplicationWindow| {
        apply_chrome_force_layers(ChromeApplyParts {
            hdr_csd_baseline: &hdr_csd,
            root: &root_fs,
            header: &header_fs,
            gl: &gl_fs,
            bar_show: &b,
            recent: &recent_fs,
            bottom: &bottom_fs,
            player: &p_fs,
        });
    })
}

/// Short timeout so GTK re-layouts windowed geometry before reading heights.
fn schedule_sub_pos_after_fs(deps: &FsNotifyDeps) {
    let gl2 = deps.widgets.gl.clone();
    let bot2 = deps.widgets.bottom.clone();
    let p2 = deps.player.clone();
    let b2 = deps.bars_shown.clone();
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
        schedule_sub_pos(&gl2, &p2, b2.get(), bot2.height())
    });
}

/// Common notify sequence after the platform-specific generation bump and leave scheduling hook.
fn fs_notify_sequence<F: FnOnce()>(
    deps: &FsNotifyDeps,
    w: &adw::ApplicationWindow,
    leave_schedule: F,
) {
    fs_notify_reset(deps);
    if w.is_fullscreen() {
        fs_notify_enter(deps, w);
    } else {
        leave_schedule();
    }
    if !w.is_fullscreen() {
        schedule_sub_pos_after_fs(deps);
    }
    fs_transition_note_notify_idle_clear(&deps.slots.fs_busy, &deps.slots.fs_settle);
}

#[cfg(target_os = "macos")]
fn fs_notify_on_event(deps: &FsNotifyDeps, w: &adw::ApplicationWindow, gen: &Rc<Cell<u32>>) {
    gen.set(gen.get().wrapping_add(1));
    fs_notify_sequence(deps, w, || fs_notify_leave(deps, w, gen));
}

#[cfg(not(target_os = "macos"))]
fn fs_notify_on_event(deps: &FsNotifyDeps, w: &adw::ApplicationWindow) {
    fs_notify_sequence(deps, w, || fs_notify_leave(deps, w));
}

fn w_in_fullscreen(ctx: &WindowInputCtx) {
    #[cfg(target_os = "macos")]
    let fs_leave_gen = Rc::new(Cell::new(0u32));

    let touch_chrome_gl = touch_chrome_gl_factory(ctx);

    wire_focus_return_repaint(ctx, Rc::clone(&touch_chrome_gl));

    let deps = FsNotifyDeps::new(ctx, touch_chrome_gl);
    let win_sig = ctx.shell.win.clone();
    win_sig.connect_fullscreened_notify(move |w| {
        #[cfg(target_os = "macos")]
        fs_notify_on_event(&deps, w, &fs_leave_gen);
        #[cfg(not(target_os = "macos"))]
        fs_notify_on_event(&deps, w);
    });
}

// --- Focus-return chrome (same touch_chrome factory as fullscreen) ---

/// Re-snapshot chrome when a window regains focus (stale gdk-macos layers vs video).
fn wire_focus_return_repaint(
    ctx: &WindowInputCtx,
    touch_chrome_gl: Rc<dyn Fn(&adw::ApplicationWindow)>,
) {
    let root_ia = ctx.shell.root.clone();
    let vh_ia = ctx.shell.video_handle.clone();
    let win_focus = ctx.shell.win.clone();
    let tch = touch_chrome_gl;
    win_focus.connect_is_active_notify(move |w| {
        if !w.is_active() {
            return;
        }
        if !w.is_fullscreen() {
            #[cfg(target_os = "macos")]
            crate::macos_window::invalidate_window_layers(w);
            return;
        }
        repaint_fullscreen_surface(&tch, &root_ia, &vh_ia, w);
    });
}

fn repaint_fullscreen_surface(
    tch: &Rc<dyn Fn(&adw::ApplicationWindow)>,
    root: &adw::ToolbarView,
    vh: &gtk::WindowHandle,
    w: &adw::ApplicationWindow,
) {
    tch(w);
    if let Some(surf) = w.native().and_then(|n| n.surface()) {
        surf.queue_render();
    }
    root.queue_allocate();
    vh.queue_draw();
    let tch2 = Rc::clone(tch);
    let w2 = w.clone();
    let _ = glib::source::idle_add_local_once(move || {
        tch2(&w2);
    });
}

include!("shell_max_mode.rs");
