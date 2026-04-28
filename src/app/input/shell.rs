fn w_in_set_shell(ctx: &WindowInputCtx) {
    let win_h = gtk::WindowHandle::new();
    win_h.set_child(Some(&ctx.ovl));
    ctx.root.add_top_bar(&ctx.header);
    ctx.root.set_content(Some(&win_h));
    ctx.root.add_bottom_bar(&ctx.bottom);
    ctx.outer_ovl.set_child(Some(&ctx.root));
    ctx.win.set_content(Some(&ctx.outer_ovl));
}

fn w_in_fullscreen(ctx: &WindowInputCtx) {
    let gl_area = &ctx.gl;
    {
        let root_fs = ctx.root.clone();
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
        let win = ctx.win.clone();
        win.connect_fullscreened_notify(move |w| {
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
            } else {
                b.set(true);
                if let Some((gw, gh)) = fr.borrow_mut().take() {
                    if w.is_maximized() {
                        w.unmaximize();
                    }
                    w.set_default_size(gw, gh);
                }
                let s = skip_fs.clone();
                let _ = glib::source::idle_add_local_once(move || {
                    s.set(false);
                });
            }
            apply_chrome(
                &root_fs,
                &gl_fs,
                &b,
                &recent_fs,
                &bottom_fs,
                &p_fs,
            );
            gl_fs.queue_render();
            w.queue_draw();
            if !w.is_fullscreen() {
                let gl2 = gl_fs.clone();
                let bot2 = bottom_fs.clone();
                let p2 = p_fs.clone();
                let b2 = b.clone();
                let _ = glib::source::idle_add_local_once(move || {
                    gl2.queue_render();
                    if let Some(bundle) = p2.borrow().as_ref() {
                        sub_prefs::apply_sub_pos_for_toolbar(
                            &bundle.mpv, b2.get(), bot2.height(), gl2.height(),
                        );
                    }
                });
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
            skip_fs.set(true);
            w.unfullscreen();
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
