fn w_in_win_motion(ctx: &WindowInputCtx) {
    let cap = gtk::EventControllerMotion::new();
    cap.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let root_c = ctx.root.clone();
        let gl_c = ctx.gl.clone();
        let recent_c = ctx.recent.clone();
        let bottom_c = ctx.bottom.clone();
        let p_c = ctx.player.clone();
        let b = ctx.bar_show.clone();
        let lcap = ctx.last_cap_xy.clone();
        let ch_hide = Rc::clone(&ctx.ch_hide);
        let sq = ctx.motion_squelch.clone();
        cap.connect_motion(glib::clone!(
            #[strong]
            root_c,
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

                if !b.get() {
                    b.set(true);
                    apply_chrome(
                        &root_c,
                        &gl_c,
                        &b,
                        &recent_c,
                        &bottom_c,
                        &p_c,
                    );
                }
                schedule_bars_autohide(Rc::clone(&ch_hide));
            }
        ));
    }
    ctx.win.add_controller(cap);
}

fn w_in_gl_motion(ctx: &WindowInputCtx) {
    let gl_c = ctx.gl.clone();
    let cur = ctx.cur_t.clone();
    let ptr = ctx.ptr_in_gl.clone();
    let sq = ctx.motion_squelch.clone();
    let lgl = ctx.last_gl_xy.clone();
    let m = gtk::EventControllerMotion::new();
    m.connect_motion(glib::clone!(
        #[strong]
        gl_c,
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
            show_pointer(&gl_c);
            replace_timeout(cur.clone(), {
                let gl2 = gl_c.clone();
                let ptr2 = ptr.clone();
                move || {
                    if ptr2.get() {
                        gl2.add_css_class("rp-cursor-hidden");
                        gl2.set_cursor_from_name(Some("none"));
                    }
                }
            });
        }
    ));
    m.connect_enter(glib::clone!(
        #[strong]
        gl_c,
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
            show_pointer(&gl_c);
            replace_timeout(cur.clone(), {
                let gl2 = gl_c.clone();
                let ptr2 = ptr.clone();
                move || {
                    if ptr2.get() {
                        gl2.add_css_class("rp-cursor-hidden");
                        gl2.set_cursor_from_name(Some("none"));
                    }
                }
            });
        }
    ));
    m.connect_leave(glib::clone!(
        #[strong]
        gl_c,
        #[strong]
        cur,
        #[strong]
        ptr,
        #[strong]
        lgl,
        move |_| {
            ptr.set(false);
            lgl.set(None);
            if let Some(id) = cur.borrow_mut().take() {
                id.remove();
            }
            show_pointer(&gl_c);
        }
    ));
    ctx.gl.add_controller(m);
}
