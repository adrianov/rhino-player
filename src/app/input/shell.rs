include!("shell_focus_return_repaint.rs");

/// Packs toolbar + video stack into the application window (safe to call once).
fn attach_window_shell(s: &WindowInputShell) {
    s.root.add_top_bar(&s.header);
    s.root.set_content(Some(&s.video_handle));
    s.root.add_bottom_bar(&s.bottom);
    s.outer_ovl.set_child(Some(&s.root));
    s.win.set_content(Some(&s.outer_ovl));
}

fn w_in_set_shell(ctx: &WindowInputCtx) {
    if ctx.shell.win.content().is_some() {
        return;
    }
    attach_window_shell(&ctx.shell);
}

fn refresh_fs_wall_clock(lbl: &gtk::Label) {
    lbl.set_label(format_wall_clock_now().as_str());
}

fn stop_fs_clock_tick(slot: &Rc<RefCell<Option<glib::SourceId>>>) {
    drop_glib_source(slot.as_ref());
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

#[cfg(not(target_os = "macos"))]
fn linux_fs_notify_maximize_now(fr: &Rc<RefCell<Option<(i32, i32)>>>, win: &adw::ApplicationWindow) {
    // [`toggle_fullscreen`] already saved geometry before `maximize()`; do not replace it here with
    // sizes read mid-transition (wrong if fullscreen notify races ahead of `is_maximized`).
    if fr.borrow().is_none() {
        *fr.borrow_mut() = Some(win_normal_size(win));
    }
    win.maximize();
}

#[cfg(target_os = "macos")]
fn macos_fs_notify_defer_maximize(fr: &Rc<RefCell<Option<(i32, i32)>>>, win: &adw::ApplicationWindow) {
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

fn w_in_fullscreen(ctx: &WindowInputCtx) {
    #[cfg(target_os = "macos")]
    let fs_leave_gen = Rc::new(Cell::new(0u32));

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
        let recent_fs = ctx.shell.recent.clone();
        let bottom_fs = ctx.shell.bottom.clone();
        let p_fs = ctx.player.clone();
        let b = ctx.bar_show.clone();
        let nav = ctx.nav_t.clone();
        let cur_fs = ctx.cur_t.clone();
        let sq = ctx.motion_squelch.clone();
        let lcap = ctx.last_cap_xy.clone();
        let lgl = ctx.last_gl_xy.clone();
        let fr = ctx.fs_restore.clone();
        let fs_pause_stash = ctx.fs_pause_stash.clone();
        let skip_fs = ctx.skip_max_to_fs.clone();
        let lu_ntf = ctx.last_unmax.clone();
        let fs_busy_ntf = Rc::clone(&ctx.fs_transition_busy);
        let fs_settle_ntf = Rc::clone(&ctx.fs_transition_settle);
        let play_fs = ctx.play_toggle.clone();
        let win_sig = ctx.shell.win.clone();
        let tch_fs = Rc::clone(&touch_chrome_gl);
        win_sig.connect_fullscreened_notify(move |w| {
            #[cfg(target_os = "macos")]
            fs_leave_gen.set(fs_leave_gen.get().wrapping_add(1));

            drop_glib_source(nav.as_ref());
            drop_glib_source(cur_fs.as_ref());
            sq.set(None);
            lcap.set(None);
            lgl.set(None);
            if w.is_fullscreen() {
                // Clear the leave-fullscreen deferral latch; stash first so we can still suppress a
                // redundant `maximize()` when this notify fires mid exit→re-enter AppKit turbulence.
                let defer_max_pair = skip_fs.get();
                skip_fs.set(false);
                // Only skip the paired `maximize` in that window — still run chrome / clock so a
                // true→false→true notify sequence does not leave stale UI if the platform emits one
                // during an AppKit transition.
                // Avoid synchronous `maximize()` in this notify on macOS: fullscreen transitions
                // can reconfigure GdkMacosMonitor's display link while frame callbacks are in
                // flight; `_gdk_macos_monitor_remove_frame_callback` may then call
                // `gdk_display_link_source_pause` when the new link is already paused (GDK CRITICAL:
                // `source->paused == FALSE`). Defer to the next main-loop turn.
                if !defer_max_pair && !w.is_maximized() {
                    #[cfg(not(target_os = "macos"))]
                    linux_fs_notify_maximize_now(&fr, w);
                    #[cfg(target_os = "macos")]
                    macos_fs_notify_defer_maximize(&fr, w);
                }
                b.set(false);
                fs_on_enter_pause(&play_fs, fs_pause_stash.as_ref());
                show_fs_wall_clock_fullscreen(&fs_clock, &fs_tick_slot, w);
                tch_fs(w);
                hide_cursor_after_bars_hide(w, &gl_fs, &recent_fs, &p_fs);
            } else {
                skip_fs.set(true);
                b.set(true);
                stop_fs_clock_tick(&fs_tick_slot);
                fs_clock.set_visible(false);
                show_chrome_pointer(w, &gl_fs);
                // Defer unmaximize + set_default_size: calling unmaximize synchronously from this
                // handler can leave `is_fullscreen()` true for one more notify cycle, which hits
                // `maximized_notify`'s "!maximized && fullscreen" path → unfullscreen again and
                // recurse until stack overflow (e.g. double-click exit).
                //
                // On macOS, `idle_add_once` still pumps mid `_NSExitFullScreenTransitionController`;
                // `apply_chrome` touches traffic-light cells (`_NSThemeZoomWidgetCell`) and can recurse
                // `_updateTitlebarContainerViewFrameIfNecessary` ↔ `_syncToolbarPosition` → stack overflow.
                // Defer restore + chrome with [`crate::fullscreen_timing::TRANSITION_SETTLE`].
                //
                let fr_leave = Rc::clone(&fr);
                let pause_leave = Rc::clone(&fs_pause_stash);
                let play_leave = play_fs.clone();
                let lu_leave = Rc::clone(&lu_ntf);
                let w_leave = w.clone();
                let skip_leave = skip_fs.clone();
                let tch_leave = Rc::clone(&tch_fs);
                #[cfg(target_os = "macos")]
                macos_schedule_leave_fs_restore_chrome(
                    &fs_leave_gen,
                    crate::fullscreen_timing::TRANSITION_SETTLE,
                    fs_leave_gen.get(),
                    fr_leave,
                    lu_leave,
                    w_leave,
                    skip_leave,
                    tch_leave,
                    play_leave,
                    pause_leave,
                );
                #[cfg(not(target_os = "macos"))]
                schedule_leave_fs_idle_linux(
                    fr_leave, lu_leave, w_leave, skip_leave, tch_leave, play_leave, pause_leave,
                );
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
            fs_transition_note_notify_idle_clear(&fs_busy_ntf, &fs_settle_ntf);
        });
    }
}

fn w_in_max_mode(ctx: &WindowInputCtx) {
    let fr = ctx.fs_restore.clone();
    let lu = ctx.last_unmax.clone();
    let skip_fs = ctx.skip_max_to_fs.clone();
    #[cfg(not(target_os = "macos"))]
    let fs_busy_mx = Rc::clone(&ctx.fs_transition_busy);
    let win = ctx.shell.win.clone();
    win.connect_maximized_notify(move |w| {
        if !w.is_maximized() && !w.is_fullscreen() {
            if !skip_fs.get() {
                *lu.borrow_mut() = win_normal_size(w);
            }
        } else if !w.is_maximized() && w.is_fullscreen() {
            // Linux: user un-maximizes while fullscreen → leave fullscreen. macOS: GDK often reports
            // `!maximized && fullscreen` during normal fullscreen entry; treating that as demaximize
            // scheduled `unfullscreen_safe` and canceled fullscreen after our idle deferral fix.
            #[cfg(not(target_os = "macos"))]
            {
                skip_fs.set(true);
                unfullscreen_safe(w, fs_busy_mx.as_ref());
            }
        } else if w.is_maximized() && !w.is_fullscreen() && !skip_fs.get() {
            if fr.borrow().is_none() {
                *fr.borrow_mut() = Some(*lu.borrow());
            }
            let w_idle = w.clone();
            let skip_idle = skip_fs.clone();
            let _ = glib::source::idle_add_local_once(move || {
                if skip_idle.get() || !w_idle.is_maximized() || w_idle.is_fullscreen() {
                    return;
                }
                #[cfg(target_os = "macos")]
                crate::macos_window::enter_fullscreen_from_maximized(&w_idle);
                #[cfg(not(target_os = "macos"))]
                w_idle.fullscreen();
            });
        }
    });
}

include!("shell_fs_restore.rs");

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
