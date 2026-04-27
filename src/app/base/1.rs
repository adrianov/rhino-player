const APP_WIN_TITLE: &str = "Rhino Player";
/// **Preferences** row for `video_smooth_60`: stores **intent**; the bundled `.vpy` runs only at ~**1.0×**.
const SMOOTH60_MENU_LABEL: &str = "Smooth Video (~60 FPS at 1.0×)";
const SEEK_BAR_MENU_LABEL: &str = "Progress Bar Preview";
const LICENSE_NOTICE: &str = concat!(
    "Rhino Player is licensed as GPL-3.0-or-later.\n\n",
    include_str!("../../../COPYRIGHT"),
    "\n\n",
    include_str!("../../../LICENSE")
);

fn title_for_open_path(path: &Path) -> String {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => format!("{name} — {APP_WIN_TITLE}"),
        None => format!("{} — {APP_WIN_TITLE}", path.display()),
    }
}
const IDLE_3S: Duration = Duration::from_secs(3);
/// After chrome hides, GTK often emits spurious pointer motion/enter; ignore for this long.
const LAYOUT_SQUELCH: Duration = Duration::from_millis(450);
/// Ignore repeated motion with the same coordinates (reflows can re-emit the same (x, y)).
const COORD_EPS: f64 = 1.0;
/// Base width (px) when fitting the window to a **horizontal** video; height follows aspect ratio.
const FIT_H_VIDEO_W: i32 = 960;
const FIT_H_VIDEO_MAX_H: i32 = 900;
/// Delay so mpv can populate `dwidth` / `dheight` (or `width` / `height`) after `loadfile`.
const FIT_WINDOW_DELAY_MS: u32 = 220;
const WARM_REVEAL_DELAY_MS: u64 = 160;
const SUB_SCAN_TICKS: u8 = 24;
const SUB_SCAN_MS: u64 = 250;
const WIN_INIT_W: i32 = 960;
const WIN_INIT_H: i32 = 540;

type RcPathFn = Rc<dyn Fn(&Path)>;
type RecentBackfillJob = (Rc<RecentContext>, Vec<PathBuf>);

fn same_xy(a: f64, b: f64) -> bool {
    (a - b).abs() < COORD_EPS
}

/// State for 3s auto-hide: header [gtk::MenuButton]s delay hiding while open (sound + subs + speed + main; audio tracks are inside the sound popover).
struct ChromeBarHide {
    nav: Rc<RefCell<Option<glib::SourceId>>>,
    vol: gtk::MenuButton,
    sub: gtk::MenuButton,
    speed: gtk::MenuButton,
    main: gtk::MenuButton,
    root: adw::ToolbarView,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::ScrolledWindow,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    squelch: Rc<Cell<Option<Instant>>>,
}

fn show_pointer(gl: &gtk::GLArea) {
    gl.remove_css_class("rp-cursor-hidden");
    gl.set_cursor_from_name(None);
}

/// Fullscreen is paired with a programmatic `maximize()` (CSD shows restore); GTK may not restore the
/// pre-maximize size after `unfullscreen` — we save **windowed** (w, h) before that maximize and
/// re-apply in `connect_fullscreened_notify` on leave.
fn win_normal_size(win: &adw::ApplicationWindow) -> (i32, i32) {
    let w = win.width();
    let h = win.height();
    if w >= 2 && h >= 2 {
        (w, h)
    } else {
        (WIN_INIT_W, WIN_INIT_H)
    }
}

fn same_open_target(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn resync_warm_continue(mpv: &Mpv) {
    let dur = mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let pos = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    if !dur.is_finite() || dur <= 0.0 || !pos.is_finite() {
        return;
    }
    let t = pos.clamp(0.0, (dur - 0.05).max(0.0));
    let s = format!("{t:.4}");
    let _ = mpv.set_property("pause", true);
    if mpv
        .command("seek", &[s.as_str(), "absolute+keyframes"])
        .is_err()
    {
        let _ = mpv.set_property("time-pos", t);
    }
}

/// `RHINO_ASPECT_DEBUG=1` — extra aspect logs (resize-end, sync poll).
fn aspect_debug() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("RHINO_ASPECT_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

/// Updates [win_aspect] from current mpv [video_display_dims] (display picture aspect, not the window’s).
fn sync_window_aspect_from_mpv(mpv: &Mpv, win_aspect: &Cell<Option<f64>>) {
    let prev = win_aspect.get();
    let dims = video_display_dims(mpv);
    if let Some((w, h)) = dims {
        if w > 0 && h > 0 {
            let r = w as f64 / h as f64;
            win_aspect.set(Some(r));
            if prev != Some(r) {
                eprintln!(
                    "[rhino] aspect: target ratio → {:.6} (from {}×{}, was {:?})",
                    r, w, h, prev
                );
            } else if aspect_debug() {
                eprintln!(
                    "[rhino] aspect: sync: dims {}×{} ratio {:.6} (unchanged)",
                    w, h, r
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

const ASPECT_MIN_W: i32 = 320;
const ASPECT_MIN_H: i32 = 200;
/// Tight: user-sized windows often sit within 0.006 of 16:9 and looked “off” with the old 0.006 tol.
const ASPECT_RESIZE_END_RATIO_TOL: f64 = 0.0002;
/// After the last [GtkWindow] size change, wait this long then apply [apply_window_video_aspect] once.
const ASPECT_RESIZE_END_DEBOUNCE: Duration = Duration::from_millis(200);

/// Minimal change from ([ww], [hh]) to match [ratio] after a user resize.
/// [ASPECT_RESIZE_END_RATIO_TOL] is stricter than the old 0.006 snap so a visible nudge is not skipped;
/// the old “±1px” no-op is dropped here (only exact integer match bails out).
fn snap_size_after_user_resize(ww: i32, hh: i32, ratio: f64) -> Option<(i32, i32)> {
    if ratio <= 0.0 || ww < 2 || hh < 2 {
        return None;
    }
    let cur = f64::from(ww) / f64::from(hh);
    if (cur - ratio).abs() <= ASPECT_RESIZE_END_RATIO_TOL {
        return None;
    }
    let w_from_h = (f64::from(hh) * ratio).round() as i32;
    let h_from_w = (f64::from(ww) / ratio).round() as i32;
    let dw = (w_from_h - ww).abs();
    let dh = (h_from_w - hh).abs();
    let (nw, nh) = if dw < dh {
        (w_from_h, hh)
    } else {
        (ww, h_from_w)
    };
    let nw = nw.clamp(ASPECT_MIN_W, 8192);
    let nh = nh.clamp(ASPECT_MIN_H, 8192);
    if nw == ww && nh == hh {
        return None;
    }
    Some((nw, nh))
}

/// One [set_default_size] to match the video [win_aspect] after user resize (see [ASPECT_RESIZE_END_DEBOUNCE]).
fn apply_window_video_aspect(
    win: &adw::ApplicationWindow,
    recent: &gtk::ScrolledWindow,
    win_aspect: &Cell<Option<f64>>,
) {
    if win.is_fullscreen() || win.is_maximized() {
        if aspect_debug() {
            eprintln!("[rhino] aspect: resize-end skip: fullscreen or maximized");
        }
        return;
    }
    if recent.is_visible() {
        if aspect_debug() {
            eprintln!("[rhino] aspect: resize-end skip: recent visible");
        }
        return;
    }
    let Some(ratio) = win_aspect.get() else {
        if aspect_debug() {
            eprintln!("[rhino] aspect: resize-end skip: no target ratio");
        }
        return;
    };
    let ww = win.width().max(2);
    let hh = win.height().max(2);
    let Some((nw, nh)) = snap_size_after_user_resize(ww, hh, ratio) else {
        if aspect_debug() {
            eprintln!(
                "[rhino] aspect: resize-end: ok {}×{} (≈{:.4} vs want {:.4})",
                ww,
                hh,
                f64::from(ww) / f64::from(hh),
                ratio
            );
        }
        return;
    };
    if aspect_debug() {
        eprintln!(
            "[rhino] aspect: resize-end: {}×{} -> {}×{} (want {:.4})",
            ww, hh, nw, nh, ratio
        );
    }
    let w2 = win.clone();
    let _ = glib::idle_add_local_once(move || {
        w2.set_default_size(nw, nh);
        w2.present();
    });
}

/// Debounced [apply_window_video_aspect] after the last width/height notify.
fn schedule_window_aspect_on_resize_end(
    deb: Rc<RefCell<Option<glib::SourceId>>>,
    win: &adw::ApplicationWindow,
    recent: &gtk::ScrolledWindow,
    win_aspect: &Rc<Cell<Option<f64>>>,
) {
    if let Some(id) = deb.borrow_mut().take() {
        id.remove();
    }
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
