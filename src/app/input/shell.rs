/// Re-snapshot chrome and the video layer when a fullscreen window regains focus.
///
/// macOS: AppKit's cross-fade across Spaces / displays leaves gdk-macos's chrome
/// `CALayer` with stale, sometimes vertically-stretched contents that appear as a
/// horizontal band of the header bar over the video. Calling `queue_allocate` +
/// `queue_draw` re-snapshots GTK chrome, [`crate::macos_window::invalidate_window_layers`]
/// drops the cached backing store on the contentView, and a deferred `touch_chrome_gl`
/// pushes a fresh video frame after AppKit's transition settles.
///
/// Linux: the same hook clears any stale Wayland header snapshot the compositor may
/// repaint over the video on focus return; mac-specific layer invalidation is a no-op.
fn wire_focus_return_repaint(
    ctx: &WindowInputCtx,
    touch_chrome_gl: Rc<dyn Fn(&adw::ApplicationWindow)>,
) {
    let root_ia = ctx.shell.root.clone();
    let vh_ia = ctx.shell.video_handle.clone();
    let win_focus = ctx.shell.win.clone();
    let tch = touch_chrome_gl;
    win_focus.connect_is_active_notify(move |w| {
        if !w.is_active() || !w.is_fullscreen() {
            return;
        }
        tch(w);
        if let Some(surf) = w.native().and_then(|n| n.surface()) {
            surf.queue_render();
        }
        root_ia.queue_allocate();
        vh_ia.queue_draw();
        #[cfg(target_os = "macos")]
        crate::macos_window::invalidate_window_layers(w);
        let tch2 = Rc::clone(&tch);
        let w2 = w.clone();
        let _ = glib::source::idle_add_local_once(move || {
            tch2(&w2);
            #[cfg(target_os = "macos")]
            crate::macos_window::invalidate_window_layers(&w2);
        });
    });
}

fn w_in_set_shell(ctx: &WindowInputCtx) {
    let s = &ctx.shell;
    s.root.add_top_bar(&s.header);
    s.root.set_content(Some(&s.video_handle));
    s.root.add_bottom_bar(&s.bottom);
    s.outer_ovl.set_child(Some(&s.root));
    s.win.set_content(Some(&s.outer_ovl));
}

fn refresh_fs_wall_clock(lbl: &gtk::Label) {
    lbl.set_label(format_wall_clock_now().as_str());
}

fn stop_fs_clock_tick(slot: &Rc<RefCell<Option<glib::SourceId>>>) {
    if let Some(id) = slot.borrow_mut().take() {
        id.remove();
    }
}

fn fs_clock_timer_step(
    wo: &adw::ApplicationWindow,
    tick_slot: &Rc<RefCell<Option<glib::SourceId>>>,
    lbl: &gtk::Label,
) -> glib::ControlFlow {
    if !wo.is_fullscreen() {
        stop_fs_clock_tick(tick_slot);
        glib::ControlFlow::Break
    } else {
        refresh_fs_wall_clock(lbl);
        glib::ControlFlow::Continue
    }
}

fn show_fs_wall_clock_fullscreen(
    lbl: &gtk::Label,
    tick_slot: &Rc<RefCell<Option<glib::SourceId>>>,
    win: &adw::ApplicationWindow,
) {
    refresh_fs_wall_clock(lbl);
    lbl.set_visible(true);
    stop_fs_clock_tick(tick_slot);
    let fc = lbl.clone();
    let fts = tick_slot.clone();
    let wo = win.clone();
    let id = glib::timeout_add_seconds_local(1, move || fs_clock_timer_step(&wo, &fts, &fc));
    *tick_slot.borrow_mut() = Some(id);
}

fn w_in_fullscreen(ctx: &WindowInputCtx) {
    let gl_area = &ctx.shell.gl;
    let fs_clock = ctx.fs_clock.clone();
    let fs_tick_slot = ctx.fs_clock_tick.clone();

    let touch_chrome_gl: Rc<dyn Fn(&adw::ApplicationWindow)> = Rc::new({
        let root_fs = ctx.shell.root.clone();
        let hdr_csd = Rc::clone(&ctx.hdr_csd_baseline);
        let header_fs = ctx.shell.header.clone();
        let gl_fs = gl_area.clone();
        let recent_fs = ctx.shell.recent.clone();
        let bottom_fs = ctx.shell.bottom.clone();
        let p_fs = ctx.player.clone();
        let b = ctx.bar_show.clone();
        move |w: &adw::ApplicationWindow| {
            apply_chrome(ChromeApplyParts {
                hdr_csd_baseline: &hdr_csd,
                root: &root_fs,
                header: &header_fs,
                gl: &gl_fs,
                bar_show: &b,
                recent: &recent_fs,
                bottom: &bottom_fs,
                player: &p_fs,
            });
            gl_fs.queue_render();
            w.queue_draw();
        }
    });

    wire_focus_return_repaint(ctx, Rc::clone(&touch_chrome_gl));

    {
        let gl_fs = gl_area.clone();
        let bottom_fs = ctx.shell.bottom.clone();
        let p_fs = ctx.player.clone();
        let b = ctx.bar_show.clone();
        let nav = ctx.nav_t.clone();
        let sq = ctx.motion_squelch.clone();
        let lcap = ctx.last_cap_xy.clone();
        let lgl = ctx.last_gl_xy.clone();
        let fr = ctx.fs_restore.clone();
        let skip_fs = ctx.skip_max_to_fs.clone();
        let win_sig = ctx.shell.win.clone();
        let tch_fs = Rc::clone(&touch_chrome_gl);
        win_sig.connect_fullscreened_notify(move |w| {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            sq.set(None);
            lcap.set(None);
            lgl.set(None);
            if w.is_fullscreen() {
                skip_fs.set(false);
                if !w.is_maximized() {
                    *fr.borrow_mut() = Some(win_normal_size(w));
                    w.maximize();
                }
                b.set(false);
                show_fs_wall_clock_fullscreen(&fs_clock, &fs_tick_slot, w);
                tch_fs(w);
            } else {
                b.set(true);
                stop_fs_clock_tick(&fs_tick_slot);
                fs_clock.set_visible(false);
                // Defer unmaximize + set_default_size: calling unmaximize synchronously from this
                // handler can leave `is_fullscreen()` true for one more notify cycle, which hits
                // `maximized_notify`'s "!maximized && fullscreen" path → unfullscreen again and
                // recurse until stack overflow (e.g. double-click exit).
                //
                // Run chrome refresh in the same idle after restore: `apply_chrome` + `queue_draw`
                // during the fullscreen→windowed transition can race gdk-macos display-link pause and
                // trip `gdk_display_link_source_pause (source->paused == FALSE)` (Gdk-CRITICAL).
                let fr_leave = Rc::clone(&fr);
                let w_leave = w.clone();
                let skip_leave = skip_fs.clone();
                let tch_leave = Rc::clone(&tch_fs);
                let _ = glib::source::idle_add_local_once(move || {
                    restore_windowed_size(&fr_leave, &w_leave);
                    skip_leave.set(false);
                    tch_leave(&w_leave);
                });
            }
            if !w.is_fullscreen() {
                let gl2 = gl_fs.clone();
                let bot2 = bottom_fs.clone();
                let p2 = p_fs.clone();
                let b2 = b.clone();
                // Short timeout so GTK re-layouts windowed geometry before reading heights.
                let _ = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(50),
                    move || schedule_sub_pos(&gl2, &p2, b2.get(), bot2.height()),
                );
            }
        });
    }
}

fn w_in_max_mode(ctx: &WindowInputCtx) {
    let fr = ctx.fs_restore.clone();
    let lu = ctx.last_unmax.clone();
    let skip_fs = ctx.skip_max_to_fs.clone();
    let win = ctx.shell.win.clone();
    win.connect_maximized_notify(move |w| {
        if !w.is_maximized() && !w.is_fullscreen() {
            *lu.borrow_mut() = win_normal_size(w);
        } else if !w.is_maximized() && w.is_fullscreen() {
            // Linux: user un-maximizes while fullscreen → leave fullscreen. macOS: GDK often reports
            // `!maximized && fullscreen` during normal fullscreen entry; treating that as demaximize
            // scheduled `unfullscreen_safe` and canceled fullscreen after our idle deferral fix.
            #[cfg(not(target_os = "macos"))]
            {
                skip_fs.set(true);
                unfullscreen_safe(w);
            }
        } else if w.is_maximized() && !w.is_fullscreen() {
            if skip_fs.get() {
                return;
            }
            if fr.borrow().is_none() {
                *fr.borrow_mut() = Some(*lu.borrow());
            }
            w.fullscreen();
        }
    });
}

fn restore_windowed_size(fr: &Rc<RefCell<Option<(i32, i32)>>>, w: &adw::ApplicationWindow) {
    if let Some((gw, gh)) = fr.borrow_mut().take() {
        if w.is_maximized() { w.unmaximize(); }
        w.set_default_size(gw, gh);
    }
}

fn schedule_sub_pos(
    gl: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    show: bool,
    bot_h: i32,
) {
    gl.queue_render();
    if let Some(bundle) = player.borrow().as_ref() {
        sub_prefs::apply_sub_pos_for_toolbar(&bundle.mpv, show, bot_h, gl.height());
    }
}
