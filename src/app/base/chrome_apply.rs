/// Bundle of refs for [`apply_chrome`].
struct ChromeApplyParts<'a, R>
where
    R: IsA<gtk::Widget>,
{
    hdr_csd_baseline: &'a Rc<Cell<Option<(bool, bool)>>>,
    root: &'a adw::ToolbarView,
    header: &'a adw::HeaderBar,
    gl: &'a gtk::GLArea,
    bar_show: &'a Rc<Cell<bool>>,
    recent: &'a R,
    bottom: &'a gtk::Box,
    player: &'a Rc<RefCell<Option<MpvBundle>>>,
}

/// Updates `ToolbarView` bar reveals, client-side decoration title slots, subtitles vs chrome, GL paint.
///
/// When the recent grid is visible, always reveal bars. When playing, visibility follows `bar_show`
/// (pointer motion clears [IDLE_3S]). Open header menus cancel auto-hide timer.
fn apply_chrome<R: IsA<gtk::Widget>>(c: ChromeApplyParts<'_, R>) {
    apply_chrome_ex(c, false);
}

/// Same as [`apply_chrome`], but always invalidates macOS window layers (fullscreen / focus return).
fn apply_chrome_force_layers<R: IsA<gtk::Widget>>(c: ChromeApplyParts<'_, R>) {
    apply_chrome_ex(c, true);
}

fn chrome_bars_show<R: IsA<gtk::Widget>>(c: &ChromeApplyParts<'_, R>) -> bool {
    let show = c.recent.is_visible() || c.bar_show.get();
    #[cfg(target_os = "macos")]
    {
        if let Some(win) = c
            .header
            .root()
            .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
        {
            crate::macos_fs_exit::heal_stuck_exit(&win);
        }
        return show && !crate::macos_fs_exit::exit_armed();
    }
    #[cfg(not(target_os = "macos"))]
    {
        show
    }
}

fn apply_chrome_ex<R: IsA<gtk::Widget>>(c: ChromeApplyParts<'_, R>, force_layers: bool) {
    c.root.set_extend_content_to_top_edge(true);
    c.root.set_extend_content_to_bottom_edge(true);
    let show = chrome_bars_show(&c);
    let reveal_changed = set_toolbar_reveal(c.root, show);
    sync_header_window_controls(c.header, c.hdr_csd_baseline, show, c.root);
    log_chrome_layout(&c, show);
    repaint_chrome_after_layout(c, show, reveal_changed || force_layers);
    if reveal_changed && show {
        transport_nudge_tick();
    }
}

fn log_chrome_layout<R: IsA<gtk::Widget>>(c: &ChromeApplyParts<'_, R>, show: bool) {
    let Some(win) =
        c.gl.root()
            .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
    else {
        return;
    };
    #[cfg(target_os = "macos")]
    {
        use glib::object::Cast;
        if let Some(shell) = c
            .bottom
            .parent()
            .and_then(|p| p.downcast::<gtk::Box>().ok())
        {
            crate::shell_debug_log::log_toolbar_layout(
                "chrome",
                &crate::shell_debug_log::ToolbarLayoutRefs {
                    win: &win,
                    root: c.root,
                    header: c.header,
                    bottom: c.bottom,
                    gl: c.gl,
                    bottom_shell: &shell,
                },
                c.recent.is_visible(),
                c.bar_show.get(),
                show,
            );
        }
    }
    #[cfg(not(target_os = "macos"))]
    crate::shell_debug_log::log_toolbar_layout(
        "chrome",
        &crate::shell_debug_log::ToolbarLayoutRefs {
            win: &win,
            root: c.root,
            header: c.header,
            bottom: c.bottom,
            gl: c.gl,
        },
        c.recent.is_visible(),
        c.bar_show.get(),
        show,
    );
}

/// Queue surface redraw; macOS layer invalidate when bars toggled or caller forced layers.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn queue_native_repaint(gl: &gtk::GLArea, invalidate_layers: bool) {
    use gtk::prelude::NativeExt;

    if let Some(win) = gl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
    {
        win.queue_draw();
        if let Some(surf) = win.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
        #[cfg(target_os = "macos")]
        if invalidate_layers && !crate::macos_header_menu::defer_layer_invalidate() {
            crate::macos_window::invalidate_window_layers(&win);
        }
    }
}

fn repaint_chrome_after_layout<R: IsA<gtk::Widget>>(
    c: ChromeApplyParts<'_, R>,
    show: bool,
    invalidate_layers: bool,
) {
    c.root.queue_allocate();
    c.gl.queue_render();
    queue_native_repaint(c.gl, invalidate_layers);
    if let Some(b) = c.player.borrow().as_ref() {
        sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, c.bottom.height(), c.gl.height());
    }
}

fn replace_timeout(s: Rc<RefCell<Option<glib::SourceId>>>, f: impl Fn() + 'static) {
    drop_glib_source(s.as_ref());
    *s.borrow_mut() = Some(glib::timeout_add_local(
        IDLE_3S,
        glib::clone!(
            #[strong]
            s,
            move || {
                *s.borrow_mut() = None;
                f();
                glib::ControlFlow::Break
            }
        ),
    ));
}

/// Bars stay shown while any header popover is open or the seek bar is grabbed.
fn bars_stay_shown(ctx: &ChromeBarHide) -> bool {
    #[cfg(target_os = "macos")]
    let pop_open = crate::macos_header_menu::any_open();
    #[cfg(not(target_os = "macos"))]
    let pop_open = false;
    ctx.vol.is_active()
        || ctx.sub.is_active()
        || ctx.speed.is_active()
        || ctx.main.is_active()
        || ctx.seek_grabbed.get()
        || pop_open
}

/// Hide bars now: re-apply chrome, squelch layout logs, hide the cursor.
fn hide_bars_now(ctx: &ChromeBarHide) {
    ctx.bar_show.set(false);
    apply_chrome(ChromeApplyParts {
        hdr_csd_baseline: &ctx.hdr_csd_baseline,
        root: &ctx.root,
        header: &ctx.header,
        gl: &ctx.gl,
        bar_show: &ctx.bar_show,
        recent: &ctx.recent,
        bottom: &ctx.bottom,
        player: &ctx.player,
    });
    ctx.squelch.set(Some(Instant::now() + LAYOUT_SQUELCH));
    hide_cursor_after_bars_hide(&ctx.win, &ctx.gl, &ctx.recent, &ctx.player);
}

fn schedule_bars_autohide(ctx: Rc<ChromeBarHide>) {
    replace_timeout(Rc::clone(&ctx.nav), {
        let ctx2 = Rc::clone(&ctx);
        move || {
            if bars_stay_shown(&ctx2) {
                schedule_bars_autohide(Rc::clone(&ctx2));
            } else {
                hide_bars_now(&ctx2);
            }
        }
    });
}
