const APP_WIN_TITLE: &str = "Rhino Player";
/// **Preferences** row for `video_smooth_60`: stores **intent**; the bundled `.vpy` runs only at ~**1.0×**.
const SMOOTH60_MENU_LABEL: &str = "Smooth Video (60 FPS)";
const SEEK_BAR_MENU_LABEL: &str = "Progress Bar Preview";
const LICENSE_NOTICE: &str = concat!(
    "Rhino Player is licensed as GPL-3.0-or-later.\n\n",
    include_str!("../../../COPYRIGHT"),
    "\n\n",
    include_str!("../../../LICENSE")
);

/// [gio::Menu] row with optional Adwaita-style symbolic icon ([ThemedIcon]),
/// mirrored to **`verb-icon`** so GTK/OS menu layers can show the same graphic.
fn menu_append_action_icon(
    menu: &gio::Menu,
    label: Option<&str>,
    detailed_action: Option<&str>,
    icon: Option<&str>,
) {
    let item = gio::MenuItem::new(label, detailed_action);
    if let Some(name) = icon {
        let themed = gio::ThemedIcon::new(name);
        item.set_icon(&themed);
        if let Some(v) = themed.serialize() {
            item.set_attribute_value("verb-icon", Some(&v));
        }
    }
    menu.append_item(&item);
}

fn title_for_open_path(path: &Path) -> String {
    crate::playback_entity::window_title_for(path)
}

/// Keeps [`gtk::ApplicationWindow::title`] and an optional GTK header-bar label aligned (macOS title
/// widget); pass `mirror` [`None`] on Linux where the shell shows the window title natively.
fn sync_app_window_title(
    win: &adw::ApplicationWindow,
    mirror: Option<&gtk::Label>,
    title: Option<&str>,
) {
    let text = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or(APP_WIN_TITLE);
    win.set_title(Some(text));
    if let Some(l) = mirror {
        l.set_label(text);
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
    win: adw::ApplicationWindow,
    root: adw::ToolbarView,
    header: adw::HeaderBar,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::Box,
    bottom: gtk::Box,
    player: Rc<RefCell<Option<MpvBundle>>>,
    squelch: Rc<Cell<Option<Instant>>>,
    /// True while the user is pressing the seek thumb. Auto-hide reschedules itself instead of
    /// hiding the bars so the slider does not vanish under the cursor mid-drag.
    seek_grabbed: Rc<Cell<bool>>,
    /// First mapped `shows_*_title_buttons` snapshot; restores exact CSD layout after chrome hide.
    hdr_csd_baseline: Rc<Cell<Option<(bool, bool)>>>,
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
    let ra = crate::video_ext::resolve_open_media_path(a);
    let rb = crate::video_ext::resolve_open_media_path(b);
    crate::video_ext::paths_same_file(&ra, &rb)
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

/// After user resize, optionally nudge outer size to [win_aspect] (see [ASPECT_RESIZE_END_DEBOUNCE]).
fn apply_window_video_aspect(
    win: &adw::ApplicationWindow,
    recent: &gtk::Box,
    win_aspect: &WinAspectCell,
) {
    if win.is_fullscreen() || win.is_maximized() {
        eprintln!("[rhino] aspect: resize-end skip fullscreen/maximized");
        return;
    }
    if recent.is_visible() {
        eprintln!("[rhino] aspect: resize-end skip recent visible");
        return;
    }
    let Some((vw, vh)) = win_aspect.get() else {
        eprintln!("[rhino] aspect: resize-end skip no target ratio");
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
    if aspect_debug() {
        let (plus_w, minus_w, plus_h, minus_h) = aspect_one_axis_deltas(ww, hh, vw, vh);
        eprintln!(
            "[rhino] aspect: one-axis deltas +W={plus_w} -W={minus_w} +H={plus_h} -H={minus_h} window={ww}×{hh}"
        );
    }
    let Some((nw, nh)) = snap_size_after_user_resize(ww, hh, vw, vh) else {
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
        return;
    };
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
