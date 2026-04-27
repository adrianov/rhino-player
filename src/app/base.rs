const APP_WIN_TITLE: &str = "Rhino Player";
/// **Preferences** row for `video_smooth_60`: stores **intent**; the bundled `.vpy` runs only at ~**1.0×**.
const SMOOTH60_MENU_LABEL: &str = "Smooth video (~60 FPS at 1.0×)";
const SEEK_BAR_MENU_LABEL: &str = "Progress bar preview";
const LICENSE_NOTICE: &str = concat!(
    "Rhino Player is licensed as GPL-3.0-or-later.\n\n",
    include_str!("../../COPYRIGHT"),
    "\n\n",
    include_str!("../../LICENSE")
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
    if !dur.is_finite() || dur <= 0.0 {
        return;
    }
    let Ok(pos) = mpv.get_property::<f64>("time-pos") else {
        return;
    };
    if !pos.is_finite() || pos < 0.12 {
        return;
    }
    let t = pos.clamp(0.0, (dur - 0.05).max(0.0));
    let s = format!("{t:.4}");
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

/// `GtkFileDialog` filter: video only (not images or “all files”).
fn video_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("Video files"));
    f.add_mime_type("video/*");
    for s in video_ext::SUFFIX {
        f.add_suffix(s);
    }
    f
}

fn vpy_file_filter() -> gtk::FileFilter {
    let f = gtk::FileFilter::new();
    f.set_name(Some("VapourSynth scripts"));
    f.add_suffix("vpy");
    f
}

fn sync_smooth_60_to_off(app: &adw::Application) {
    if let Some(a) = app.lookup_action("smooth-60") {
        a.change_state(&false.to_variant());
    }
}

/// Rebuilds the **Preferences** submenu: Smooth 60, seek preview, optional `basename` for `video_vs_path`
/// ([vs-custom]), [choose-vs].
fn video_pref_submenu_rebuild(m: &gio::Menu, p: &db::VideoPrefs, app: &adw::Application) {
    m.remove_all();
    m.append(Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"));
    m.append(Some(SEEK_BAR_MENU_LABEL), Some("app.seek-bar-preview"));
    if !p.vs_path.trim().is_empty() {
        let name = std::path::Path::new(p.vs_path.trim())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("script.vpy");
        m.append(Some(name), Some("app.vs-custom"));
    }
    m.append(
        Some("Choose VapourSynth script (.vpy)…"),
        Some("app.choose-vs"),
    );
    if let Some(a) = app
        .lookup_action("vs-custom")
        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
    {
        a.set_state(&(!p.vs_path.trim().is_empty()).to_variant());
    }
}

/// Main menu: [db::VideoPrefs] and `app.*` actions for `gio::Menu` (before [win::present]).
fn register_video_app_actions(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    gl_area: &gtk::GLArea,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video_pref: Rc<RefCell<db::VideoPrefs>>,
    pref_menu: &gio::Menu,
    seek_bar_on: Rc<Cell<bool>>,
) {
    let v0 = video_pref.borrow().clone();
    let app_s = app.clone();
    let smooth_60 = gio::SimpleAction::new_stateful("smooth-60", None, &v0.smooth_60.to_variant());
    {
        let p = Rc::clone(&video_pref);
        let pl = Rc::clone(player);
        let gla = gl_area.clone();
        smooth_60.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(b) = s.get::<bool>() else {
                return;
            };
            a.set_state(s);
            {
                let mut g = p.borrow_mut();
                g.smooth_60 = b;
                db::save_video(&g);
            }
            if let Some(plr) = pl.borrow().as_ref() {
                let off = {
                    let mut g = p.borrow_mut();
                    video_pref::apply_mpv_video(&plr.mpv, &mut g, None)
                }
                .smooth_auto_off;
                if off {
                    sync_smooth_60_to_off(&app_s);
                }
            }
            gla.queue_render();
        });
    }
    app.add_action(&smooth_60);

    let seek_bar_preview =
        gio::SimpleAction::new_stateful("seek-bar-preview", None, &seek_bar_on.get().to_variant());
    {
        let on = Rc::clone(&seek_bar_on);
        seek_bar_preview.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(b) = s.get::<bool>() else {
                return;
            };
            a.set_state(s);
            on.set(b);
            db::save_seek_bar_preview(b);
        });
    }
    app.add_action(&seek_bar_preview);

    let vs_custom = gio::SimpleAction::new_stateful(
        "vs-custom",
        None,
        &(!v0.vs_path.trim().is_empty()).to_variant(),
    );
    {
        let p = Rc::clone(&video_pref);
        let pl = Rc::clone(player);
        let gla = gl_area.clone();
        let app_c = app.clone();
        let pref = pref_menu.clone();
        vs_custom.connect_change_state(move |a, s| {
            let Some(s) = s else {
                return;
            };
            let Some(checked) = s.get::<bool>() else {
                return;
            };
            a.set_state(s);
            if checked {
                return;
            }
            {
                let mut g = p.borrow_mut();
                if g.vs_path.trim().is_empty() {
                    return;
                }
                g.vs_path.clear();
                db::save_video(&g);
            }
            if let Some(plr) = pl.borrow().as_ref() {
                let off = {
                    let mut g = p.borrow_mut();
                    video_pref::apply_mpv_video(&plr.mpv, &mut g, None)
                }
                .smooth_auto_off;
                if off {
                    sync_smooth_60_to_off(&app_c);
                }
            }
            video_pref_submenu_rebuild(&pref, &p.borrow(), &app_c);
            gla.queue_render();
        });
    }
    app.add_action(&vs_custom);

    let choose = gio::SimpleAction::new("choose-vs", None);
    {
        let app2 = app.clone();
        let w = win.clone();
        let p = Rc::clone(&video_pref);
        let pl = Rc::clone(player);
        let gla = gl_area.clone();
        let pref = pref_menu.clone();
        choose.connect_activate(move |_, _| {
            let vf = vpy_file_filter();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&vf);
            let dialog = gtk::FileDialog::builder()
                .title("VapourSynth script")
                .modal(true)
                .filters(&filters)
                .default_filter(&vf)
                .build();
            let app3 = app2.clone();
            let p2 = p.clone();
            let pl2 = Rc::clone(&pl);
            let gl2 = gla.clone();
            let pref2 = pref.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else {
                    return;
                };
                let Some(path) = file.path() else {
                    eprintln!("[rhino] choose-vs: path required");
                    return;
                };
                {
                    let mut g = p2.borrow_mut();
                    g.vs_path = path.to_str().unwrap_or("").to_string();
                    g.smooth_60 = true;
                    db::save_video(&g);
                }
                if let Some(plr) = pl2.borrow().as_ref() {
                    let off = {
                        let mut g = p2.borrow_mut();
                        video_pref::apply_mpv_video(&plr.mpv, &mut g, None)
                    }
                    .smooth_auto_off;
                    if off {
                        sync_smooth_60_to_off(&app3);
                    } else if let Some(sa) = app3
                        .lookup_action("smooth-60")
                        .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
                    {
                        sa.set_state(&p2.borrow().smooth_60.to_variant());
                    }
                } else if let Some(sa) = app3
                    .lookup_action("smooth-60")
                    .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
                {
                    sa.set_state(&true.to_variant());
                }
                video_pref_submenu_rebuild(&pref2, &p2.borrow(), &app3);
                gl2.queue_render();
            });
        });
    }
    app.add_action(&choose);
    video_pref_submenu_rebuild(pref_menu, &v0, app);
}

/// Fullscreen and **maximized** are tied so the titlebar restore / unmaximize control matches
/// fullscreen. The **titlebar maximize** action only maximizes first; `connect_maximized_notify` then
/// calls `fullscreen()` so the same control always ends in true fullscreen.
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
    skip_max_to_fs: &Cell<bool>,
) {
    if win.is_fullscreen() {
        skip_max_to_fs.set(true);
        win.unfullscreen();
        // unmaximize + set_default_size run in `connect_fullscreened_notify` (leave) if `fs_restore` was set
    } else if !win.is_maximized() {
        *fs_restore.borrow_mut() = Some(win_normal_size(win));
        win.maximize();
        // Fullscreen is applied in `connect_maximized_notify` (maximized && !fullscreen).
    } else {
        if fs_restore.borrow().is_none() {
            *fs_restore.borrow_mut() = Some(*last_unmax.borrow());
        }
        win.fullscreen();
    }
}

/// `AdwToolbarView` top and bottom bars float over the `GLArea` (windowed and fullscreen).
/// When the recent grid is visible, always reveal bars. When playing, visibility follows
/// `bar_show` (set true on pointer motion; cleared after [IDLE_3S] of no motion on the window).
/// Open header [gtk::MenuButton]s (main menu, sound/volume popover) skip that hide: any open menu
/// cancels the pending auto-hide, and a timer that would hide while a menu is open is rescheduled.
fn apply_chrome(
    root: &adw::ToolbarView,
    gl: &gtk::GLArea,
    bar_show: &Cell<bool>,
    recent: &impl IsA<gtk::Widget>,
    bottom: &gtk::Box,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    root.set_extend_content_to_top_edge(true);
    root.set_extend_content_to_bottom_edge(true);
    let show = if recent.is_visible() {
        true
    } else {
        bar_show.get()
    };
    root.set_reveal_top_bars(show);
    root.set_reveal_bottom_bars(show);
    gl.queue_render();
    if let Some(b) = player.borrow().as_ref() {
        sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bottom.height(), gl.height());
    }
}

fn replace_timeout(s: Rc<RefCell<Option<glib::SourceId>>>, f: impl Fn() + 'static) {
    if let Some(id) = s.borrow_mut().take() {
        id.remove();
    }
    *s.borrow_mut() = Some(glib::timeout_add_local(
        IDLE_3S,
        glib::clone!(
            #[strong]
            s,
            move || {
                *s.borrow_mut() = None;
                f();
                glib::ControlFlow::Break
            }
        ),
    ));
}

fn schedule_bars_autohide(ctx: Rc<ChromeBarHide>) {
    replace_timeout(Rc::clone(&ctx.nav), {
        let ctx2 = Rc::clone(&ctx);
        move || {
            if ctx2.vol.is_active()
                || ctx2.sub.is_active()
                || ctx2.speed.is_active()
                || ctx2.main.is_active()
            {
                schedule_bars_autohide(Rc::clone(&ctx2));
            } else {
                ctx2.bar_show.set(false);
                apply_chrome(
                    &ctx2.root,
                    &ctx2.gl,
                    &ctx2.bar_show,
                    &ctx2.recent,
                    &ctx2.bottom,
                    &ctx2.player,
                );
                ctx2.squelch.set(Some(Instant::now() + LAYOUT_SQUELCH));
            }
        }
    });
}

/// Clicks to another header [gtk::MenuButton] are blocked while a **modal** popover is open.
/// [gtk::Popover:modal] on GTK 4.14+ — set to false so the rest of the window (including
/// the other header buttons) stays clickable; [gtk::Popover:autohide] still dismisses on outside press.
fn header_popover_non_modal(pop: &gtk::Popover) {
    if pop.find_property("modal").is_none() {
        return;
    }
    pop.set_property("modal", false);
}

/// No built-in “menu button group.” Before the [gtk::MenuButton] default: close other menus,
/// then an idle [set_active] if the first press did not open the target (e.g. lost to popover stack).
fn header_menubtns_switch(menus: [gtk::MenuButton; 4]) {
    for (i, menu) in menus.iter().enumerate() {
        let g = gtk::GestureClick::new();
        g.set_button(gtk::gdk::BUTTON_PRIMARY);
        g.set_propagation_limit(gtk::PropagationLimit::None);
        g.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = menu.clone();
        let sibs: Vec<gtk::MenuButton> = menus
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, b)| b.clone())
            .collect();
        let c = this.clone();
        g.connect_pressed(move |_, n, _, _| {
            if n != 1 {
                return;
            }
            let had_other = sibs.iter().any(|b| b.is_active());
            for b in &sibs {
                b.set_active(false);
            }
            if had_other && !c.is_active() {
                let t = c.clone();
                glib::idle_add_local(move || {
                    if !t.is_active() {
                        t.set_active(true);
                    }
                    glib::ControlFlow::Break
                });
            }
        });
        this.add_controller(g);
    }
}

/// Display (or stream) size in pixels from mpv, if known.
fn video_display_dims(mpv: &Mpv) -> Option<(i64, i64)> {
    let pair = |mw: &Mpv, wk: &str, hk: &str| {
        let w = mw.get_property::<i64>(wk).ok()?;
        let h = mw.get_property::<i64>(hk).ok()?;
        (w > 0 && h > 0).then_some((w, h))
    };
    pair(mpv, "dwidth", "dheight").or_else(|| pair(mpv, "width", "height"))
}

fn window_size_for_horizontal_video(vw: i64, vh: i64) -> (i32, i32) {
    let wf = vw as f64;
    let hf = vh as f64;
    let mut nw = FIT_H_VIDEO_W;
    let mut nh = (FIT_H_VIDEO_W as f64 * hf / wf).round() as i32;
    if nh > FIT_H_VIDEO_MAX_H {
        nh = FIT_H_VIDEO_MAX_H;
        nw = (FIT_H_VIDEO_MAX_H as f64 * wf / hf).round() as i32;
    }
    nw = nw.clamp(320, 4096);
    nh = nh.clamp(200, 4096);
    (nw, nh)
}

/// Resize the window to match a **landscape** video aspect (wider than tall). No-op in fullscreen, when maximized, for portrait or square, or if dimensions are unknown.
fn schedule_window_fit_h_video(
    player: Rc<RefCell<Option<MpvBundle>>>,
    win: adw::ApplicationWindow,
) {
    let w = win.clone();
    let _ = glib::timeout_add_local(
        Duration::from_millis(u64::from(FIT_WINDOW_DELAY_MS)),
        move || {
            if w.is_fullscreen() || w.is_maximized() {
                return glib::ControlFlow::Break;
            }
            let b = match player.try_borrow() {
                Ok(b) => b,
                Err(_) => return glib::ControlFlow::Break,
            };
            let Some(pl) = b.as_ref() else {
                return glib::ControlFlow::Break;
            };
            let Some((px, py)) = video_display_dims(&pl.mpv) else {
                return glib::ControlFlow::Break;
            };
            if px <= py {
                return glib::ControlFlow::Break;
            }
            let (nw, nh) = window_size_for_horizontal_video(px, py);
            w.set_default_size(nw, nh);
            glib::ControlFlow::Break
        },
    );
}

fn schedule_or_defer_recent_backfill(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    pending: &Rc<RefCell<Option<RecentBackfillJob>>>,
    ctx: Rc<RecentContext>,
    paths: Vec<PathBuf>,
) {
    if player.borrow().is_some() {
        recent_view::schedule_thumb_backfill(ctx, paths);
    } else {
        *pending.borrow_mut() = Some((ctx, paths));
    }
}

fn drain_recent_backfill(pending: &Rc<RefCell<Option<RecentBackfillJob>>>) {
    if let Some((ctx, paths)) = pending.borrow_mut().take() {
        recent_view::schedule_thumb_backfill(ctx, paths);
    }
}

fn schedule_sub_button_scan(player: Rc<RefCell<Option<MpvBundle>>>, button: gtk::MenuButton) {
    button.set_visible(false);
    let tries = Rc::new(Cell::new(0u8));
    let _ = glib::timeout_add_local(Duration::from_millis(SUB_SCAN_MS), move || {
        let has_subs = player
            .borrow()
            .as_ref()
            .is_some_and(|b| sub_tracks::has_subtitle_tracks(&b.mpv));
        button.set_visible(has_subs);
        if has_subs {
            return glib::ControlFlow::Break;
        }
        let next = tries.get().saturating_add(1);
        tries.set(next);
        if next >= SUB_SCAN_TICKS {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn preload_first_continue(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    video: &Rc<RefCell<db::VideoPrefs>>,
    recent: &impl IsA<gtk::Widget>,
) -> Option<bool> {
    let has_file = player
        .borrow()
        .as_ref()
        .and_then(|b| local_file_from_mpv(&b.mpv))
        .is_some();
    if !recent.is_visible() || has_file {
        return None;
    }
    let path = history::load().into_iter().next()?;
    let mut p = player.borrow_mut();
    let b = p.as_mut()?;
    let _ = b.mpv.set_property("pause", true);
    b.load_file_path(&path, false).ok()?;
    let _ = b.mpv.set_property("pause", true);
    Some(video_pref::apply_mpv_video(&b.mpv, &mut video.borrow_mut(), None).smooth_auto_off)
}

pub fn run() -> i32 {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }

    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return 1;
    }

    // Without HANDLES_OPEN, the desktop/portal rejects opening files: "This application can not open files"
    // (https://github.com/gtk-rs/gtk4-rs/issues/1039) — `open` is used instead of argv[1].
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|_app| {
        icons::register_hicolor_from_manifest();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        db::init();
        theme::apply();
    });

    let player: Rc<RefCell<Option<MpvBundle>>> = Rc::new(RefCell::new(None));
    // Queued for first GL init ([connect_realize]) or applied via [on_open] when libmpv is ready.
    let file_boot: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let on_open_slot: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    {
        let fb = Rc::clone(&file_boot);
        let slot = Rc::clone(&on_open_slot);
        let p_open = Rc::clone(&player);
        // With HANDLES_OPEN, the default handler does **not** emit `activate` when argv lists files —
        // only `open` (see g_application_run: files → `open` signal). Without a call to
        // `Gio::Application::activate`, no window is created and the process exits (use count 0).
        app.connect_open(move |app, files, _| {
            let path = match files.first().and_then(|f| f.path()) {
                Some(p) => p,
                None => return,
            };
            if p_open.borrow().is_some() {
                if let Some(f) = slot.borrow().as_ref() {
                    f(&path);
                } else {
                    *fb.borrow_mut() = Some(path);
                }
                return;
            }
            *fb.borrow_mut() = Some(path);
            if app.windows().is_empty() {
                app.activate();
            }
        });
    }
    {
        let p = player.clone();
        let file_boot = Rc::clone(&file_boot);
        let on_open_slot = Rc::clone(&on_open_slot);
        app.connect_activate(move |a: &adw::Application| {
            if a.windows().is_empty() {
                if file_boot.borrow().is_none() {
                    if let Some(arg) = std::env::args().nth(1) {
                        *file_boot.borrow_mut() = Some(PathBuf::from(arg));
                    }
                }
                build_window(a, &p, Rc::clone(&file_boot), Rc::clone(&on_open_slot));
            }
        });
    }
    app.run().into()
}
