// Window-aspect sync from mpv plus the debounced resize-end outer-size snap.
// Split out of constants_and_window_aspect.rs (include!'d into the same module scope).

/// Updates [win_aspect] from mpv coded size when available (stable across `vf`); else display dims.
fn sync_window_aspect_from_mpv(mpv: &Mpv, win_aspect: &WinAspectCell) {
    let prev = win_aspect.get();
    let dims = video_snap_aspect_dims(mpv);
    if let Some((w, h)) = dims {
        if w > 0 && h > 0 {
            let next = (w, h);
            win_aspect.set(Some(next));
            if prev != Some(next) {
                let r = win_aspect_ratio(next);
                eprintln!(
                    "[rhino] aspect: target ratio → {:.6} (from {}×{}, was {:?})",
                    r,
                    w,
                    h,
                    prev.map(|(pw, ph)| win_aspect_ratio((pw, ph)))
                );
            }
        } else if aspect_debug() {
            eprintln!(
                "[rhino] aspect: sync: non-positive display dims {}×{}",
                w, h
            );
        }
    } else if aspect_debug() {
        eprintln!(
            "[rhino] aspect: sync: video_display_dims() is None (mpv dwidth/dheight, width/height not set?)"
        );
    }
}

/// After the last [GtkWindow] size change, wait this long then apply [apply_window_video_aspect] once.
const ASPECT_RESIZE_END_DEBOUNCE: Duration = Duration::from_millis(200);

fn log_one_axis_deltas(ww: i32, hh: i32, vw: i64, vh: i64) {
    if !aspect_debug() {
        return;
    }
    let (plus_w, minus_w, plus_h, minus_h) = aspect_one_axis_deltas(ww, hh, vw, vh);
    eprintln!(
        "[rhino] aspect: one-axis deltas +W={plus_w} -W={minus_w} +H={plus_h} -H={minus_h} window={ww}×{hh}"
    );
}

fn log_resize_end_keep(ww: i32, hh: i32, vw: i64, vh: i64) {
    let (w_off, h_off) = aspect_dim_offsets(ww, hh, vw, vh);
    eprintln!(
        "[rhino] aspect: resize-end keep {}×{} rel_err={:.5} w_off={:.2} h_off={:.2} video={}×{}",
        ww,
        hh,
        aspect_rel_err(ww, hh, vw, vh),
        w_off,
        h_off,
        vw,
        vh
    );
}

/// Snap branch: log the picked axis, mark the resize programmatic, then re-apply outer size idle.
fn apply_resize_end_snap(
    win: &adw::ApplicationWindow,
    nw: i32,
    nh: i32,
    ww: i32,
    hh: i32,
    vw: i64,
    vh: i64,
) {
    let pick = if nw > ww {
        "+W"
    } else if nw < ww {
        "-W"
    } else if nh > hh {
        "+H"
    } else {
        "-H"
    };
    eprintln!(
        "[rhino] aspect: resize-end snap {}×{} -> {}×{} pick={pick} (video {}×{})",
        ww, hh, nw, nh, vw, vh
    );
    note_programmatic_win_resize(nw, nh);
    let w2 = win.clone();
    let _ = glib::idle_add_local_once(move || {
        if !apply_window_outer_size(&w2, nw, nh) {
            eprintln!(
                "[rhino] aspect: resize-end apply noop gtk already {}×{}",
                w2.width(),
                w2.height()
            );
        }
    });
}

/// Skip reasons for the resize-end snap: fullscreen/maximized, recent grid visible, no target.
fn resize_end_skip_reason(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &WinAspectCell,
) -> bool {
    if win.is_fullscreen() || win.is_maximized() {
        eprintln!("[rhino] aspect: resize-end skip fullscreen/maximized");
        return true;
    }
    if recent.is_visible() {
        eprintln!("[rhino] aspect: resize-end skip recent visible");
        return true;
    }
    if win_aspect.get().is_none() {
        eprintln!("[rhino] aspect: resize-end skip no target ratio");
        return true;
    }
    false
}

/// After user resize, optionally nudge outer size to [win_aspect] (see [ASPECT_RESIZE_END_DEBOUNCE]).
fn apply_window_video_aspect(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &WinAspectCell,
) {
    if resize_end_skip_reason(win, recent, win_aspect) {
        return;
    }
    let Some((vw, vh)) = win_aspect.get() else {
        return;
    };
    let ww = win.width().max(2);
    let hh = win.height().max(2);
    if skip_resize_end_snap(ww, hh, vw, vh) {
        if aspect_debug() {
            eprintln!("[rhino] aspect: resize-end skip programmatic {ww}×{hh}");
        }
        return;
    }
    log_one_axis_deltas(ww, hh, vw, vh);
    match snap_size_after_user_resize(ww, hh, vw, vh) {
        Some((nw, nh)) => apply_resize_end_snap(win, nw, nh, ww, hh, vw, vh),
        None => log_resize_end_keep(ww, hh, vw, vh),
    }
}

/// Debounced [apply_window_video_aspect] after the last width/height notify.
fn schedule_window_aspect_on_resize_end(
    deb: Rc<RefCell<Option<glib::SourceId>>>,
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &Rc<WinAspectCell>,
) {
    drop_glib_source(deb.as_ref());
    let d = Rc::clone(&deb);
    let w = win.clone();
    let r = recent.clone();
    let wa = Rc::clone(win_aspect);
    *deb.borrow_mut() = Some(glib::timeout_add_local(
        ASPECT_RESIZE_END_DEBOUNCE,
        glib::clone!(
            #[strong]
            d,
            move || {
                *d.borrow_mut() = None;
                apply_window_video_aspect(&w, &r, wa.as_ref());
                glib::ControlFlow::Break
            }
        ),
    ));
}
