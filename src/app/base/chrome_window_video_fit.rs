// Landscape fit-on-open (window outer size = video aspect; chrome overlays the GLArea).

thread_local! {
    static FIT_DEB: RefCell<Option<glib::SourceId>> = RefCell::new(None);
}

fn apply_window_fit_size(win: &adw::ApplicationWindow, nw: i32, nh: i32) -> bool {
    win.set_default_size(nw, nh);
    let needs_resize = win.width() != nw || win.height() != nh;
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
    crate::shell_debug_log::log_fit(nw, nh, win, (px, py));
    let resized = apply_window_fit_size(win, nw, nh);
    #[cfg(target_os = "macos")]
    if !resized {
        schedule_shell_layout_after_gtk_resize(nw, nh);
    }
    #[cfg(not(target_os = "macos"))]
    if resized {
        schedule_shell_layout_sync();
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
