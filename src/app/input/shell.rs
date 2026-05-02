fn w_in_set_shell(ctx: &WindowInputCtx) {
    ctx.root.add_top_bar(&ctx.header);
    ctx.root.set_content(Some(&ctx.video_handle));
    ctx.root.add_bottom_bar(&ctx.bottom);
    ctx.outer_ovl.set_child(Some(&ctx.root));
    ctx.win.set_content(Some(&ctx.outer_ovl));
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
    let gl_area = &ctx.gl;
    let fs_clock = ctx.fs_clock.clone();
    let fs_tick_slot = ctx.fs_clock_tick.clone();
    {
        let root_fs = ctx.root.clone();
        let hdr_csd = Rc::clone(&ctx.hdr_csd_baseline);
        let header_fs = ctx.header.clone();
        let gl_fs = gl_area.clone();
        let recent_fs = ctx.recent.clone();
        let bottom_fs = ctx.bottom.clone();
        let p_fs = ctx.player.clone();
        let b = ctx.bar_show.clone();
        let nav = ctx.nav_t.clone();
        let sq = ctx.motion_squelch.clone();
        let lcap = ctx.last_cap_xy.clone();
        let lgl = ctx.last_gl_xy.clone();
        let fr = ctx.fs_restore.clone();
        let skip_fs = ctx.skip_max_to_fs.clone();
        let win_sig = ctx.win.clone();
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
            } else {
                b.set(true);
                stop_fs_clock_tick(&fs_tick_slot);
                fs_clock.set_visible(false);
                restore_windowed_size(&fr, w);
                let s = skip_fs.clone();
                let _ = glib::source::idle_add_local_once(move || { s.set(false); });
            }
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
    let win = ctx.win.clone();
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
