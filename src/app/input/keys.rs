fn w_in_key_controller(ctx: &WindowInputCtx) {
    let p = ctx.player.clone();
    let win_key = ctx.win.clone();
    let recent_esc = ctx.recent.clone();
    let flow_esc = ctx.flow_recent.clone();
    let gl_esc = ctx.gl.clone();
    let op_esc = ctx.on_open.clone();
    let rem_esc = ctx.on_remove.clone();
    let trash_esc = ctx.on_trash.clone();
    let rbf_esc = ctx.recent_backfill.clone();
    let last_esc = ctx.last_path.clone();
    let seof_esc = ctx.sibling_seof.clone();
    let nav_esc = ctx.sibling_nav.clone();
    let video_chrome_key = ctx.on_video_chrome.clone();
    let browse_esc = ctx.browse_chrome.clone();
    let fr_key = ctx.fs_restore.clone();
    let lu_key = ctx.last_unmax.clone();
    let skip_key = ctx.skip_max_to_fs.clone();
    let wa_esc = ctx.win_aspect.clone();
    let ush_k = ctx.undo_shell.clone();
    let ula_k = ctx.undo_label.clone();
    let uti_k = ctx.undo_timer.clone();
    let ur_k = ctx.undo_remove_stack.clone();
    let undo_t_esc = ctx.undo_btn.clone();
    let play_key = PlayToggleCtx {
        app: ctx.app.clone(),
        player: p.clone(),
        video_pref: Rc::clone(&ctx.video_pref),
        win: win_key.clone(),
        gl: gl_esc.clone(),
        recent: recent_esc.clone(),
        last_path: last_esc.clone(),
        on_video_chrome: video_chrome_key,
        win_aspect: wa_esc.clone(),
        sub_menu: None,
    };
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
                    sibling_nav: nav_esc.clone(),
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
        if !toggle_play_pause(&play_key) {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    });
    ctx.win.add_controller(k);
}
