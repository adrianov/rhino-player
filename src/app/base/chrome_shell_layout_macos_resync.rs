// macOS resync after geometry changes: GTK-resize polling, surface-size compositing
// debounce, and refresh hooks for window resize / recent-hide (not Smooth vf — unchanged geometry).
//
// Included from `chrome_shell_layout.rs` (macOS only); shares its module scope.

/// Wait until GTK client size matches `(target_w, target_h)` then run [`schedule_shell_layout_sync`].
#[cfg(target_os = "macos")]
pub(crate) fn schedule_shell_layout_after_gtk_resize(target_w: i32, target_h: i32) {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    poll_shell_layout_after_resize(Rc::clone(&ctx), target_w, target_h, 0);
}

/// Run the shell sync and nudge compositing width once GTK reports the target size,
/// or after [`SHELL_RESIZE_POLL_MAX_ATTEMPTS`] failed attempts.
#[cfg(target_os = "macos")]
fn poll_shell_layout_after_resize(
    ctx: Rc<ShellLayoutCtx>,
    target_w: i32,
    target_h: i32,
    attempt: u8,
) {
    const SHELL_RESIZE_POLL_MAX_ATTEMPTS: u8 = 20;
    let gw = ctx.win.width();
    let gh = ctx.win.height();
    if gw == target_w && gh == target_h {
        crate::shell_debug_log::log(format!(
            "gtk synced {gw}x{gh} → shell sync (attempt={attempt})"
        ));
        finish_poll_shell_layout(&ctx);
        return;
    }
    if attempt >= SHELL_RESIZE_POLL_MAX_ATTEMPTS {
        crate::shell_debug_log::log(format!(
            "gtk sync timeout gtk={gw}x{gh} target={target_w}x{target_h} → shell sync anyway"
        ));
        finish_poll_shell_layout(&ctx);
        return;
    }
    let c = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(16), move || {
        poll_shell_layout_after_resize(c, target_w, target_h, attempt + 1);
    });
}

#[cfg(target_os = "macos")]
fn finish_poll_shell_layout(ctx: &Rc<ShellLayoutCtx>) {
    schedule_shell_layout_sync();
    crate::macos_window::nudge_gdk_compositing_width(&ctx.win);
}

/// Debounce slot so surface size storms trigger at most one compositing refresh.
#[cfg(target_os = "macos")]
type CompositingDebounce = Rc<RefCell<Option<glib::SourceId>>>;

/// Build the debounced refresh closure for a surface's width/height notifications.
#[cfg(target_os = "macos")]
fn compositing_refresh_scheduler(deb: CompositingDebounce) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if crate::macos_header_menu::defer_layer_invalidate() {
            return;
        }
        if deb.borrow().is_some() {
            return;
        }
        let deb2 = Rc::clone(&deb);
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(32), move || {
            *deb2.borrow_mut() = None;
            refresh_registered_shell_compositing();
        });
        *deb.borrow_mut() = Some(id);
    })
}

/// Wire width/height notifies of the mapped window surface to the debounced refresh.
#[cfg(target_os = "macos")]
fn wire_surface_size_refresh(surf: &gtk::gdk::Surface, deb: CompositingDebounce) {
    use gtk::gdk::prelude::SurfaceExt;

    let schedule = compositing_refresh_scheduler(deb);
    let on_w = Rc::clone(&schedule);
    surf.connect_width_notify(move |_| on_w());
    surf.connect_height_notify(move |_| schedule());
}

#[cfg(target_os = "macos")]
pub(crate) fn wire_macos_surface_compositing_refresh(ctx: &Rc<ShellLayoutCtx>) {
    use gtk::prelude::NativeExt;

    let deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let win = ctx.win.clone();
    win.connect_map(move |w| {
        let Some(surf) = w.native().and_then(|n| n.surface()) else {
            return;
        };
        wire_surface_size_refresh(&surf, Rc::clone(&deb));
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn wire_macos_recent_hide_refresh(
    _win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::Box,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    let gl2 = gl.clone();
    let p = Rc::clone(player);
    recent.connect_notify_local(Some("visible"), move |r, _| {
        if r.is_visible() {
            return;
        }
        refresh_registered_shell_compositing();
        if let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) {
            sync_shell_layout_tag(&ctx, "recent-hide");
            crate::macos_window::nudge_gdk_compositing_width(&ctx.win);
        }
        if let Ok(g) = p.try_borrow() {
            if let Some(b) = g.as_ref() {
                b.nudge_shell_layout_after_resize(&gl2);
            }
        }
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn refresh_registered_shell_compositing() {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    crate::macos_window::refresh_gdk_shell_compositing(
        &ctx.win,
        &ctx.gl,
        &ctx.header,
        &ctx.root,
        &ctx.bottom_shell,
        &ctx.bottom,
    );
}
