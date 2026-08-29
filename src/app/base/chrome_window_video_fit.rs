// Landscape fit-on-open (first launch / small window only). Later loads keep user size.
// Smooth same-media reload must not re-fit — see [suppress_window_fit_for_load].

thread_local! {
    static FIT_DEB: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
    /// [MpvBundle::warm_file_gen] of a same-media Smooth reload — skip fit until a newer gen.
    static FIT_SUPPRESS_GEN: Cell<Option<u32>> = const { Cell::new(None) };
}

const FIT_INIT_SIZE_TOL: i32 = 16;

/// Cancel a pending landscape fit and skip fit for this load generation (Smooth same-media reload).
pub(crate) fn suppress_window_fit_for_load(load_gen: u32) {
    FIT_DEB.with(drop_glib_source);
    FIT_SUPPRESS_GEN.set(Some(load_gen));
}

/// True when [schedule_window_fit_h_video] must no-op for the open player's load generation.
fn fit_suppressed_for_gen(load_gen: Option<u32>) -> bool {
    let Some(suppressed) = FIT_SUPPRESS_GEN.get() else {
        return false;
    };
    match load_gen {
        Some(cur) if cur == suppressed => true,
        Some(_) => {
            // A newer `loadfile` replaced the suppressed Smooth reload.
            FIT_SUPPRESS_GEN.set(None);
            false
        }
        None => false,
    }
}

/// True when the shell is still at the default size or smaller than the landscape fit target.
fn should_landscape_fit_on_load(win: &adw::ApplicationWindow, fit_w: i32, fit_h: i32) -> bool {
    let ww = win.width();
    let hh = win.height();
    if ww < 2 || hh < 2 {
        return true;
    }
    let near_init = (ww - WIN_INIT_W).abs() <= FIT_INIT_SIZE_TOL
        && (hh - WIN_INIT_H).abs() <= FIT_INIT_SIZE_TOL;
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

/// Landscape fit-on-open branch: resize to the target and resync the shell layout per platform.
fn apply_landscape_fit_on_open(win: &adw::ApplicationWindow, nw: i32, nh: i32, dims: (i64, i64)) {
    let ww = win.width().max(2);
    let hh = win.height().max(2);
    crate::shell_debug_log::log_fit(nw, nh, win, dims);
    eprintln!("[rhino] aspect: fit-on-open {ww}×{hh} -> {nw}×{nh}");
    let resized = apply_window_outer_size(win, nw, nh);
    #[cfg(target_os = "macos")]
    if !resized {
        schedule_shell_layout_after_gtk_resize(nw, nh);
    }
    #[cfg(not(target_os = "macos"))]
    if resized {
        schedule_shell_layout_sync();
    }
}

/// Window is already past the fit target: keep its size, maybe nudge onto the video aspect.
fn nudge_after_landscape_skip(
    mpv: &Mpv,
    win: &adw::ApplicationWindow,
    nw: i32,
    nh: i32,
    fallback_dims: (i64, i64),
) {
    let ww = win.width().max(2);
    let hh = win.height().max(2);
    let (vw, vh) = video_snap_aspect_dims(mpv).unwrap_or(fallback_dims);
    eprintln!("[rhino] aspect: fit-on-open skip keep {ww}×{hh} (landscape target {nw}×{nh})");
    if let Some((sw, sh)) = snap_size_after_user_resize(ww, hh, vw, vh) {
        eprintln!("[rhino] aspect: load nudge {ww}×{hh} -> {sw}×{sh}");
        if apply_window_outer_size(win, sw, sh) {
            #[cfg(not(target_os = "macos"))]
            schedule_shell_layout_sync();
        }
    }
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
    if should_landscape_fit_on_load(win, nw, nh) {
        apply_landscape_fit_on_open(win, nw, nh, (px, py));
        return;
    }
    nudge_after_landscape_skip(&pl.mpv, win, nw, nh, (px, py));
}

/// Resize the window to match a **landscape** video aspect (chrome overlays; no extra height).
/// Honours [suppress_window_fit_for_load] (Smooth same-media reload).
fn schedule_window_fit_h_video(
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
    gl: gtk::GLArea,
) {
    let load_gen = player.borrow().as_ref().map(MpvBundle::warm_file_gen);
    if fit_suppressed_for_gen(load_gen) {
        eprintln!(
            "[rhino] aspect: fit-on-open skip (suppressed gen={})",
            load_gen.unwrap_or(0)
        );
        FIT_DEB.with(drop_glib_source);
        return;
    }
    FIT_DEB.with(drop_glib_source);
    let id = glib::timeout_add_local(
        std::time::Duration::from_millis(u64::from(FIT_WINDOW_DELAY_MS)),
        move || fire_window_fit_h_video(&player, &win, &gl),
    );
    FIT_DEB.with(|deb| *deb.borrow_mut() = Some(id));
}

/// Debounce tick: clear the pending source, then apply the landscape fit once.
fn fire_window_fit_h_video(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
) -> glib::ControlFlow {
    FIT_DEB.with(|d| *d.borrow_mut() = None);
    apply_window_fit_h_video(player, win, gl);
    glib::ControlFlow::Break
}
