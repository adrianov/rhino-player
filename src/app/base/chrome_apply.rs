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
    c.root.set_extend_content_to_top_edge(true);
    c.root.set_extend_content_to_bottom_edge(true);
    let show = c.recent.is_visible() || c.bar_show.get();
    let reveal_changed = set_toolbar_reveal(c.root, show);
    sync_header_window_controls(c.header, c.hdr_csd_baseline, show, c.root);
    log_chrome_layout(&c, show);
    repaint_chrome_after_layout(c, show);
    // Thumb updates are skipped while bars stay hidden; catch up as soon as they show.
    if reveal_changed && show {
        transport_nudge_tick();
    }
}

fn log_chrome_layout<R: IsA<gtk::Widget>>(c: &ChromeApplyParts<'_, R>, show: bool) {
    let Some(win) = c
        .gl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
    else {
        return;
    };
    #[cfg(target_os = "macos")]
    {
        use glib::object::Cast;
        if let Some(shell) = c.bottom.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
            crate::shell_debug_log::log_toolbar_layout(
                "chrome",
                &win,
                c.root,
                c.header,
                c.bottom,
                c.gl,
                c.recent.is_visible(),
                c.bar_show.get(),
                show,
                &shell,
            );
        }
    }
    #[cfg(not(target_os = "macos"))]
    crate::shell_debug_log::log_toolbar_layout(
        "chrome",
        &win,
        c.root,
        c.header,
        c.bottom,
        c.gl,
        c.recent.is_visible(),
        c.bar_show.get(),
        show,
    );
}

fn repaint_chrome_after_layout<R: IsA<gtk::Widget>>(c: ChromeApplyParts<'_, R>, show: bool) {
    use gtk::prelude::NativeExt;

    c.root.queue_allocate();
    c.gl.queue_render();
    if let Some(win) = c
        .gl
        .root()
        .and_then(|r| r.downcast::<adw::ApplicationWindow>().ok())
    {
        win.queue_draw();
        if let Some(surf) = win.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
        #[cfg(target_os = "macos")]
        if !crate::macos_header_menu::defer_layer_invalidate() {
            crate::macos_window::invalidate_window_layers(&win);
        }
    }
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

fn schedule_bars_autohide(ctx: Rc<ChromeBarHide>) {
    replace_timeout(Rc::clone(&ctx.nav), {
        let ctx2 = Rc::clone(&ctx);
        move || {
            #[cfg(target_os = "macos")]
            let pop_open = crate::macos_header_menu::any_open();
            #[cfg(not(target_os = "macos"))]
            let pop_open = false;
            if ctx2.vol.is_active()
                || ctx2.sub.is_active()
                || ctx2.speed.is_active()
                || ctx2.main.is_active()
                || ctx2.seek_grabbed.get()
                || pop_open
            {
                schedule_bars_autohide(Rc::clone(&ctx2));
            } else {
                ctx2.bar_show.set(false);
                apply_chrome(ChromeApplyParts {
                    hdr_csd_baseline: &ctx2.hdr_csd_baseline,
                    root: &ctx2.root,
                    header: &ctx2.header,
                    gl: &ctx2.gl,
                    bar_show: &ctx2.bar_show,
                    recent: &ctx2.recent,
                    bottom: &ctx2.bottom,
                    player: &ctx2.player,
                });
                ctx2.squelch.set(Some(Instant::now() + LAYOUT_SQUELCH));
                hide_cursor_after_bars_hide(&ctx2.win, &ctx2.gl, &ctx2.recent, &ctx2.player);
            }
        }
    });
}
