struct WindowInputCtx {
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    ovl: gtk::Overlay,
    bottom: gtk::Box,
    gl: gtk::GLArea,
    recent: gtk::ScrolledWindow,
    flow_recent: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    bar_show: Rc<Cell<bool>>,
    nav_t: Rc<RefCell<Option<glib::SourceId>>>,
    cur_t: Rc<RefCell<Option<glib::SourceId>>>,
    ptr_in_gl: Rc<Cell<bool>>,
    motion_squelch: Rc<Cell<Option<Instant>>>,
    last_cap_xy: Rc<Cell<Option<(f64, f64)>>>,
    last_gl_xy: Rc<Cell<Option<(f64, f64)>>>,
    fs_restore: Rc<RefCell<Option<(i32, i32)>>>,
    skip_max_to_fs: Rc<Cell<bool>>,
    last_unmax: Rc<RefCell<(i32, i32)>>,
    ch_hide: Rc<ChromeBarHide>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    browse_chrome: Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
}

fn wire_window_input(ctx: WindowInputCtx) {
    let WindowInputCtx {
        win,
        root,
        header,
        ovl,
        bottom,
        gl: gl_area,
        recent: recent_scrl,
        flow_recent,
        player,
        bar_show,
        nav_t,
        cur_t,
        ptr_in_gl,
        motion_squelch,
        last_cap_xy,
        last_gl_xy,
        fs_restore,
        skip_max_to_fs,
        last_unmax,
        ch_hide,
        on_open,
        on_remove,
        on_trash,
        recent_backfill,
        last_path,
        sibling_seof,
        browse_chrome,
        win_aspect,
        undo_shell,
        undo_label,
        undo_btn,
        undo_timer,
        undo_remove_stack,
    } = ctx;

    let win_h = gtk::WindowHandle::new();
    win_h.set_child(Some(&ovl));

    root.add_top_bar(&header);
    root.set_content(Some(&win_h));
    root.add_bottom_bar(&bottom);

    win.set_content(Some(&root));

    {
        let root_fs = root.clone();
        let gl_fs = gl_area.clone();
        let recent_fs = recent_scrl.clone();
        let bottom_fs = bottom.clone();
        let p_fs = player.clone();
        let b = bar_show.clone();
        let nav = nav_t.clone();
        let sq = motion_squelch.clone();
        let lcap = last_cap_xy.clone();
        let lgl = last_gl_xy.clone();
        let fr = fs_restore.clone();
        let skip_fs = skip_max_to_fs.clone();
        win.connect_fullscreened_notify(move |w| {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            sq.set(None);
            lcap.set(None);
            lgl.set(None);
            // Entering fullscreen: hide chrome until the user moves. Leaving fullscreen: show chrome and
            // force redraw — always clearing `bar_show` on both transitions left a hidden-ToolbarView
            // state that could paint a full-screen black layer behind a restored windowed frame (GNOME).
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
                // Do not `skip_max_to_fs = false` here. `unfullscreen` is often followed in the same
                // event batch by `connect_maximized_notify` with (maximized && !fullscreen), which
                // would call `fullscreen()` again if we already cleared the skip flag. Clear on idle
                // after that notify runs.
                let s = skip_fs.clone();
                let _ = glib::source::idle_add_local_once(move || {
                    s.set(false);
                });
            }
            apply_chrome(&root_fs, &gl_fs, &b, &recent_fs, &bottom_fs, &p_fs);
            gl_fs.queue_render();
            w.queue_draw();
            if !w.is_fullscreen() {
                let gl2 = gl_fs.clone();
                let _ = glib::source::idle_add_local_once(move || {
                    gl2.queue_render();
                });
            }
        });
    }

    // Titlebar maximize (or any path that sets maximized without fullscreen) → fullscreen; keep
    // `last_unmax` for restore when `fs_restore` is still empty. Unmax while still fullscreen (some
    // WMs) → `unfullscreen()`. Restore on leave stays in `connect_fullscreened_notify`.
    {
        let fr = fs_restore.clone();
        let lu = last_unmax.clone();
        let skip_fs = skip_max_to_fs.clone();
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

    {
        let root_c = root.clone();
        let gl_c = gl_area.clone();
        let recent_c = recent_scrl.clone();
        let bottom_c = bottom.clone();
        let p_c = player.clone();
        let b = bar_show.clone();
        let sq = motion_squelch.clone();
        let lcap = last_cap_xy.clone();
        let cap = gtk::EventControllerMotion::new();
        cap.set_propagation_phase(gtk::PropagationPhase::Capture);
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

                b.set(true);
                apply_chrome(&root_c, &gl_c, &b, &recent_c, &bottom_c, &p_c);
                schedule_bars_autohide(Rc::clone(&ch_hide));
            }
        ));
        win.add_controller(cap);
    }

    {
        let gl_c = gl_area.clone();
        let cur = cur_t.clone();
        let ptr = ptr_in_gl.clone();
        let sq = motion_squelch.clone();
        let lgl = last_gl_xy.clone();
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
        gl_area.add_controller(m);
    }

    {
        let p = player.clone();
        let win_key = win.clone();
        let recent_esc = recent_scrl.clone();
        let flow_esc = flow_recent.clone();
        let gl_esc = gl_area.clone();
        let op_esc = on_open.clone();
        let rem_esc = on_remove.clone();
        let trash_esc = on_trash.clone();
        let rbf_esc = recent_backfill.clone();
        let last_esc = last_path.clone();
        let seof_esc = sibling_seof.clone();
        let browse_esc = browse_chrome.clone();
        let fr_key = fs_restore.clone();
        let lu_key = last_unmax.clone();
        let skip_key = skip_max_to_fs.clone();
        let wa_esc = win_aspect.clone();
        let ush_k = undo_shell.clone();
        let ula_k = undo_label.clone();
        let uti_k = undo_timer.clone();
        let ur_k = undo_remove_stack.clone();
        let undo_t_esc = undo_btn.clone();
        let k = gtk::EventControllerKey::new();
        k.connect_key_pressed(move |_, key, _code, _m| {
            if key == gtk::gdk::Key::Escape {
                if win_key.is_fullscreen() {
                    skip_key.set(true);
                    win_key.unfullscreen();
                    return glib::Propagation::Stop;
                }
                if recent_esc.is_visible() {
                    return glib::Propagation::Stop;
                }
                if p.borrow().is_none() {
                    return glib::Propagation::Stop;
                }
                back_to_browse(
                    &BackToBrowseCtx {
                        player: p.clone(),
                        on_open: op_esc.clone(),
                        on_remove: rem_esc.clone(),
                        on_trash: trash_esc.clone(),
                        recent_backfill: rbf_esc.clone(),
                        last_path: last_esc.clone(),
                        sibling_seof: seof_esc.clone(),
                        win_aspect: wa_esc.clone(),
                        on_browse: browse_esc.clone(),
                        undo_shell: ush_k.clone(),
                        undo_label: ula_k.clone(),
                        undo_btn: undo_t_esc.clone(),
                        undo_timer: uti_k.clone(),
                        undo_remove_stack: ur_k.clone(),
                    },
                    &win_key,
                    &gl_esc,
                    &recent_esc,
                    &flow_esc,
                    true,
                );
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
                toggle_fullscreen(&win_key, &fr_key, &lu_key, &skip_key);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::m || key == gtk::gdk::Key::M {
                let g = p.borrow();
                let Some(b) = g.as_ref() else {
                    return glib::Propagation::Proceed;
                };
                let muted = b.mpv.get_property::<bool>("mute").unwrap_or(false);
                if b.mpv.set_property("mute", !muted).is_err() {
                    return glib::Propagation::Proceed;
                }
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Up {
                let g = p.borrow();
                let Some(b) = g.as_ref() else {
                    return glib::Propagation::Proceed;
                };
                nudge_mpv_volume(&b.mpv, 5.0);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Down {
                let g = p.borrow();
                let Some(b) = g.as_ref() else {
                    return glib::Propagation::Proceed;
                };
                nudge_mpv_volume(&b.mpv, -5.0);
                return glib::Propagation::Stop;
            }
            if key != gtk::gdk::Key::space {
                return glib::Propagation::Proceed;
            }
            let g = p.borrow();
            let Some(b) = g.as_ref() else {
                return glib::Propagation::Proceed;
            };
            let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
            if b.mpv.set_property("pause", !paused).is_err() {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
        win.add_controller(k);
    }
}
