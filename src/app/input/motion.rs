#[cfg(target_os = "macos")]
include!("motion_macos_unfocused.rs");

fn w_in_win_motion(ctx: &WindowInputCtx) {
    let cap = gtk::EventControllerMotion::new();
    cap.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let root_c = ctx.shell.root.clone();
        let hdr_csd = Rc::clone(&ctx.hdr_csd_baseline);
        let hdr_c = ctx.shell.header.clone();
        let gl_c = ctx.shell.gl.clone();
        let recent_c = ctx.shell.recent.clone();
        let bottom_c = ctx.shell.bottom.clone();
        let p_c = ctx.player.clone();
        let b = ctx.bar_show.clone();
        let lcap = ctx.last_cap_xy.clone();
        let ch_hide = Rc::clone(&ctx.ch_hide);
        let sq = ctx.motion_squelch.clone();
        let win_c = ctx.shell.win.clone();
        cap.connect_motion(glib::clone!(
            #[strong]
            win_c,
            #[strong]
            root_c,
            #[strong]
            hdr_csd,
            #[strong]
            hdr_c,
            #[strong]
            gl_c,
            #[strong]
            recent_c,
            #[strong]
            bottom_c,
            #[strong]
            p_c,
            #[strong]
            b,
            #[strong]
            lcap,
            #[strong]
            ch_hide,
            #[strong]
            sq,
            move |_, x, y| {
                if recent_c.is_visible() {
                    return;
                }
                if let Some(t) = sq.get() {
                    if Instant::now() < t {
                        return;
                    }
                }
                if let Some((lx, ly)) = lcap.get() {
                    if same_xy(x, lx) && same_xy(y, ly) {
                        return;
                    }
                }
                lcap.set(Some((x, y)));
                show_chrome_pointer(&win_c, &gl_c);

                if !b.get() {
                    b.set(true);
                    apply_chrome(ChromeApplyParts {
                        hdr_csd_baseline: &hdr_csd,
                        root: &root_c,
                        header: &hdr_c,
                        gl: &gl_c,
                        bar_show: &b,
                        recent: &recent_c,
                        bottom: &bottom_c,
                        player: &p_c,
                    });
                }
                schedule_bars_autohide(Rc::clone(&ch_hide));
            }
        ));
    }
    ctx.shell.win.add_controller(cap);
}

fn w_in_gl_motion(ctx: &WindowInputCtx) {
    let gl_c = ctx.shell.gl.clone();
    let win_m = ctx.shell.win.clone();
    let p_m = ctx.player.clone();
    let cur = ctx.cur_t.clone();
    let ptr = ctx.ptr_in_gl.clone();
    let sq = ctx.motion_squelch.clone();
    let lgl = ctx.last_gl_xy.clone();
    let m = gtk::EventControllerMotion::new();
    m.connect_motion(glib::clone!(
        #[strong]
        gl_c,
        #[strong]
        win_m,
        #[strong]
        p_m,
        #[strong]
        cur,
        #[strong]
        ptr,
        #[strong]
        sq,
        #[strong]
        lgl,
        move |_, x, y| {
            ptr.set(true);
            if let Some(t) = sq.get() {
                if Instant::now() < t {
                    return;
                }
            }
            if let Some((lx, ly)) = lgl.get() {
                if same_xy(x, lx) && same_xy(y, ly) {
                    return;
                }
            }
            lgl.set(Some((x, y)));
            show_chrome_pointer(&win_m, &gl_c);
            replace_timeout(cur.clone(), {
                let gl2 = gl_c.clone();
                let win2 = win_m.clone();
                let player2 = p_m.clone();
                let ptr2 = ptr.clone();
                move || {
                    if ptr2.get() {
                        apply_theater_cursor_hide(&win2, &gl2, &player2);
                    }
                }
            });
        }
    ));
    m.connect_enter(glib::clone!(
        #[strong]
        gl_c,
        #[strong]
        win_m,
        #[strong]
        p_m,
        #[strong]
        cur,
        #[strong]
        ptr,
        #[strong]
        sq,
        move |_, _x, _y| {
            ptr.set(true);
            if let Some(t) = sq.get() {
                if Instant::now() < t {
                    return;
                }
            }
            show_chrome_pointer(&win_m, &gl_c);
            replace_timeout(cur.clone(), {
                let gl2 = gl_c.clone();
                let win2 = win_m.clone();
                let player2 = p_m.clone();
                let ptr2 = ptr.clone();
                move || {
                    if ptr2.get() {
                        apply_theater_cursor_hide(&win2, &gl2, &player2);
                    }
                }
            });
        }
    ));
    m.connect_leave(glib::clone!(
        #[strong]
        gl_c,
        #[strong]
        win_m,
        #[strong]
        cur,
        #[strong]
        ptr,
        #[strong]
        lgl,
        move |_| {
            ptr.set(false);
            lgl.set(None);
            // Slot may already be cleared in [`shell::w_in_fullscreen`] before synthesized leave.
            drop_glib_source(cur.as_ref());
            show_chrome_pointer(&win_m, &gl_c);
        }
    ));
    ctx.shell.gl.add_controller(m);
    #[cfg(target_os = "macos")]
    wire_macos_gl_cursor_while_unfocused(ctx);
}
