// Reveal continue-strip hide + unpause after open.

/// Hides recent grid and kicks off playback (immediate or delayed warm reveal).
/// Always raises the window so openings from external handlers (e.g. file manager while in background)
/// bring the UI to the foreground.
fn reveal_ui_after_load(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    warm_hit: bool,
) {
    win.present();
    let delayed_warm = warm_hit && o.play_on_start;
    if !delayed_warm {
        recent_layer.set_visible(false);
        if let Some(pf) = o.playback_focus.as_ref() {
            pf.set(true);
        }
        if let Some(f) = o.on_start.as_ref() {
            f();
        }
        #[cfg(target_os = "macos")]
        {
            crate::app::refresh_registered_shell_compositing();
            crate::macos_window::nudge_gdk_compositing_width(win);
            if let Some(b) = player.borrow().as_ref() {
                b.nudge_shell_layout_after_resize(gl);
            }
        }
    }
    gl.queue_render();
    if o.play_on_start {
        start_playback(player, win, gl, recent_layer, o, delayed_warm);
    }
    if let Some(b) = player.borrow().as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    }
    schedule_window_fit_h_video(Rc::clone(player), win.clone(), gl.clone());
}

/// Unpauses mpv; for warm-hit paths, delays reveal by [WARM_REVEAL_DELAY_MS].
fn start_playback(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
    delayed_warm: bool,
) {
    if let Some(b) = player.borrow().as_ref() {
        b.set_skip_media_persist(false);
    }
    if delayed_warm {
        let recent = recent_layer.as_ref().clone();
        let win2 = win.clone();
        let gl2 = gl.clone();
        let player2 = player.clone();
        let on_start = o.on_start.clone();
        let playback_focus = o.playback_focus.clone();
        let _ = glib::timeout_add_local(Duration::from_millis(WARM_REVEAL_DELAY_MS), move || {
            recent.set_visible(false);
            if let Some(pf) = playback_focus.as_ref() {
                pf.set(true);
            }
            if let Some(f) = on_start.as_ref() { f(); }
            win2.present();
            unpause_and_finish_resume(&player2);
            gl2.queue_render();
            glib::ControlFlow::Break
        });
    } else {
        win.present();
        unpause_and_finish_resume(player);
        gl.queue_render();
    }
}

