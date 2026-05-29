// Landscape fit-on-open (first launch / small window only). Later loads keep user size.

thread_local! {
    static FIT_DEB: RefCell<Option<glib::SourceId>> = RefCell::new(None);
}

const FIT_INIT_SIZE_TOL: i32 = 16;

/// True when the shell is still at the default size or smaller than the landscape fit target.
fn should_landscape_fit_on_load(win: &adw::ApplicationWindow, fit_w: i32, fit_h: i32) -> bool {
    let ww = win.width();
    let hh = win.height();
    if ww < 2 || hh < 2 {
        return true;
    }
    let near_init =
        (ww - WIN_INIT_W).abs() <= FIT_INIT_SIZE_TOL && (hh - WIN_INIT_H).abs() <= FIT_INIT_SIZE_TOL;
    near_init || (ww <= fit_w && hh <= fit_h)
}

/// Apply outer window size on an already-visible toplevel (`set_default_size` alone is not enough).
pub(crate) fn apply_window_outer_size(win: &adw::ApplicationWindow, nw: i32, nh: i32) -> bool {
    win.set_default_size(nw, nh);
    let needs_resize = win.width() != nw || win.height() != nh;
    if needs_resize {
        note_programmatic_win_resize(nw, nh);
    }
    if !needs_resize {
        crate::shell_debug_log::log(format!(
            "fit skip gtk already {nw}x{nh} ({}x{})",
            win.width(),
            win.height()
        ));
        return false;
    }
    #[cfg(target_os = "macos")]
    crate::macos_window::resize_window_frame(win, nw, nh);
    #[cfg(not(target_os = "macos"))]
    {
        use gtk::gdk::prelude::SurfaceExt;
        use gtk::prelude::NativeExt;

        win.queue_resize();
        if let Some(surf) = win.native().and_then(|n| n.surface()) {
            surf.request_layout();
        }
        win.queue_allocate();
        win.present();
    }
    true
}

fn apply_window_fit_h_video(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    _gl: &gtk::GLArea,
) {
    if win.is_fullscreen() || win.is_maximized() {
        return;
    }
    let b = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = b.as_ref() else {
        return;
    };
    let Some((px, py)) = video_display_dims(&pl.mpv) else {
        return;
    };
    if px <= py {
        return;
    }
    let (nw, nh) = window_size_for_horizontal_video(px, py);
    let ww = win.width().max(2);
    let hh = win.height().max(2);
    if should_landscape_fit_on_load(win, nw, nh) {
        crate::shell_debug_log::log_fit(nw, nh, win, (px, py));
        eprintln!(
            "[rhino] aspect: fit-on-open {}×{} -> {}×{}",
            ww, hh, nw, nh
        );
        let resized = apply_window_outer_size(win, nw, nh);
        #[cfg(target_os = "macos")]
        if !resized {
            schedule_shell_layout_after_gtk_resize(nw, nh);
        }
        #[cfg(not(target_os = "macos"))]
        if resized {
            schedule_shell_layout_sync();
        }
        return;
    }
    let (vw, vh) = video_snap_aspect_dims(&pl.mpv).unwrap_or((px, py));
    eprintln!(
        "[rhino] aspect: fit-on-open skip keep {}×{} (landscape target {}×{})",
        ww, hh, nw, nh
    );
    if let Some((sw, sh)) = snap_size_after_user_resize(ww, hh, vw, vh) {
        eprintln!("[rhino] aspect: load nudge {}×{} -> {}×{}", ww, hh, sw, sh);
        if apply_window_outer_size(win, sw, sh) {
            #[cfg(not(target_os = "macos"))]
            schedule_shell_layout_sync();
        }
    }
}

/// Resize the window to match a **landscape** video aspect (chrome overlays; no extra height).
fn schedule_window_fit_h_video(
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
) {
    FIT_DEB.with(|deb| drop_glib_source(deb));
    let p = Rc::clone(&player);
    let w = win.clone();
    let gl_t = gl.clone();
    let id = glib::timeout_add_local(
        std::time::Duration::from_millis(u64::from(FIT_WINDOW_DELAY_MS)),
        move || {
            FIT_DEB.with(|d| *d.borrow_mut() = None);
            apply_window_fit_h_video(&p, &w, &gl_t);
            glib::ControlFlow::Break
        },
    );
    FIT_DEB.with(|deb| *deb.borrow_mut() = Some(id));
}
