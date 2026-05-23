// After programmatic window resize (VOB / DVD fit-on-open), gdk-macos needs the same
// relayout + layer invalidation as a manual resize or fullscreen focus return.

thread_local! {
    static SHELL_LAYOUT: RefCell<Option<Rc<ShellLayoutCtx>>> = RefCell::new(None);
}

/// Widget refs for shell relayout after geometry changes (registered once when attached).
pub(crate) struct ShellLayoutCtx {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    video_handle: gtk::WindowHandle,
    gl: gtk::GLArea,
    bottom: gtk::Box,
    #[cfg(target_os = "macos")]
    bottom_shell: gtk::Box,
    recent: gtk::Box,
    bar_show: Rc<Cell<bool>>,
    player: Rc<RefCell<Option<MpvBundle>>>,
    touch_chrome: RefCell<Option<Rc<dyn Fn()>>>,
}

pub(crate) fn register_shell_layout(ctx: Rc<ShellLayoutCtx>) {
    SHELL_LAYOUT.with(|s| *s.borrow_mut() = Some(ctx));
}

pub(crate) fn wire_shell_layout_chrome(touch: Rc<dyn Fn()>) {
    SHELL_LAYOUT.with(|s| {
        if let Some(ctx) = s.borrow().as_ref() {
            *ctx.touch_chrome.borrow_mut() = Some(touch);
        }
    });
}

fn toolbar_show(ctx: &ShellLayoutCtx) -> bool {
    #[cfg(not(target_os = "macos"))]
    return ctx.recent.is_visible() || ctx.bar_show.get();
    #[cfg(target_os = "macos")]
    {
        let mut show = ctx.recent.is_visible() || ctx.bar_show.get();
        if crate::macos_fs_exit::exit_armed() {
            show = true;
        }
        show
    }
}

/// Wait until GTK client size matches `(target_w, target_h)` then run [`schedule_shell_layout_sync`].
#[cfg(target_os = "macos")]
pub(crate) fn schedule_shell_layout_after_gtk_resize(target_w: i32, target_h: i32) {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    poll_shell_layout_after_resize(Rc::clone(&ctx), target_w, target_h, 0);
}

#[cfg(target_os = "macos")]
fn poll_shell_layout_after_resize(ctx: Rc<ShellLayoutCtx>, target_w: i32, target_h: i32, attempt: u8) {
    let gw = ctx.win.width();
    let gh = ctx.win.height();
    if gw == target_w && gh == target_h {
        crate::shell_debug_log::log(format!(
            "gtk synced {gw}x{gh} → shell sync (attempt={attempt})"
        ));
        schedule_shell_layout_sync();
        crate::macos_window::nudge_gdk_compositing_width(&ctx.win);
        return;
    }
    if attempt >= 20 {
        crate::shell_debug_log::log(format!(
            "gtk sync timeout gtk={gw}x{gh} target={target_w}x{target_h} → shell sync anyway"
        ));
        schedule_shell_layout_sync();
        crate::macos_window::nudge_gdk_compositing_width(&ctx.win);
        return;
    }
    let c = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(16), move || {
        poll_shell_layout_after_resize(c, target_w, target_h, attempt + 1);
    });
}

fn sync_shell_layout_tag(ctx: &ShellLayoutCtx, tag: &str) {
    #[cfg(not(target_os = "macos"))]
    use gtk::prelude::NativeExt;

    let show = toolbar_show(ctx);
    let _ = set_toolbar_reveal(&ctx.root, show);
    ctx.win.queue_resize();
    ctx.root.queue_allocate();
    ctx.root.queue_draw();
    ctx.header.queue_draw();
    ctx.bottom.queue_draw();
    #[cfg(target_os = "macos")]
    ctx.bottom_shell.queue_draw();
    ctx.video_handle.queue_draw();
    ctx.gl.queue_render();
    #[cfg(target_os = "macos")]
    {
        crate::macos_bottom_bar::repaint_opaque(&ctx.bottom_shell, &ctx.bottom);
        crate::macos_window::refresh_gdk_shell_compositing(
            &ctx.win,
            &ctx.gl,
            &ctx.header,
            &ctx.root,
            &ctx.bottom_shell,
            &ctx.bottom,
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        ctx.win.queue_draw();
        if let Some(surf) = ctx.win.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
    }
    if let Ok(g) = ctx.player.try_borrow() {
        if let Some(b) = g.as_ref() {
            b.nudge_shell_layout_after_resize(&ctx.gl);
            sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, ctx.bottom.height(), ctx.gl.height());
        }
    }
    crate::shell_debug_log::log_toolbar_layout(
        tag,
        &ctx.win,
        &ctx.root,
        &ctx.header,
        &ctx.bottom,
        &ctx.gl,
        ctx.recent.is_visible(),
        ctx.bar_show.get(),
        show,
        #[cfg(target_os = "macos")]
        &ctx.bottom_shell,
    );
}

/// Idle + short delays so ToolbarView bottom bar lands after NSWindow / revealer layout.
pub(crate) fn schedule_shell_layout_sync() {
    let Some(ctx) = SHELL_LAYOUT.with(|s| s.borrow().clone()) else {
        return;
    };
    sync_shell_layout_tag(&ctx, "sched-0");
    let c1 = Rc::clone(&ctx);
    let _ = glib::idle_add_local_once(move || sync_shell_layout_tag(&c1, "sched-idle"));
    let c2 = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
        sync_shell_layout_tag(&c2, "sched-50ms");
    });
    let c3 = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
        sync_shell_layout_tag(&c3, "sched-150ms");
        if let Some(touch) = c3.touch_chrome.borrow().clone() {
            touch();
        }
    });
    let c4 = Rc::clone(&ctx);
    let _ = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
        sync_shell_layout_tag(&c4, "sched-300ms");
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn wire_macos_surface_compositing_refresh(ctx: &Rc<ShellLayoutCtx>) {
    use gtk::gdk::prelude::SurfaceExt;
    use gtk::prelude::NativeExt;

    let deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let win = ctx.win.clone();
    win.connect_map(move |w| {
        let Some(surf) = w.native().and_then(|n| n.surface()) else {
            return;
        };
        let deb_w = Rc::clone(&deb);
        let schedule = Rc::new(move || {
            if crate::macos_header_menu::defer_layer_invalidate() {
                return;
            }
            if deb_w.borrow().is_some() {
                return;
            }
            let deb2 = Rc::clone(&deb_w);
            let id = glib::timeout_add_local_once(std::time::Duration::from_millis(32), move || {
                *deb2.borrow_mut() = None;
                refresh_registered_shell_compositing();
            });
            *deb_w.borrow_mut() = Some(id);
        });
        let on_w = Rc::clone(&schedule);
        surf.connect_width_notify(move |_| on_w());
        let on_h = schedule;
        surf.connect_height_notify(move |_| on_h());
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

/// After VapourSynth `vf add` (runs after initial shell sync on DVD open), refresh gdk chrome.
#[cfg(target_os = "macos")]
pub(crate) fn schedule_macos_shell_refresh_after_vf() {
    refresh_registered_shell_compositing();
    let win = SHELL_LAYOUT.with(|s| s.borrow().as_ref().map(|c| c.win.clone()));
    if let Some(win) = win {
        let _ = glib::idle_add_local_once(move || {
            refresh_registered_shell_compositing();
            crate::macos_window::nudge_gdk_compositing_width(&win);
        });
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn schedule_macos_shell_refresh_after_vf() {}
