use adw::prelude::*;
use gio::prelude::{
    ActionExt as GioActionExt, ActionMapExt as GioActionMapExt, ApplicationExtManual, FileExt,
};
use glib::prelude::{ObjectExt, ToVariant};
use gtk::gio;
use gtk::glib;
use gtk::prelude::{ActionableExt, EventControllerExt, GestureExt, GtkWindowExt, NativeExt, WidgetExt};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::audio_tracks;
use crate::continue_undo::{apply as apply_bar_undo, ContinueBarUndo};
use crate::db;
use crate::sub_prefs;
use crate::sub_tracks;
use crate::format_time;
use crate::history;
use crate::idle_inhibit;
use crate::video_ext;
use libmpv2::Mpv;
use crate::icons;

use crate::media_probe::{
    capture_list_remove_undo, card_data_list, is_done_enough_to_drop_continue, local_file_from_mpv,
    remove_continue_entry, CardData,
};
use crate::trash_xdg;
use crate::mpv_embed::MpvBundle;
use crate::recent_view;
use crate::recent_view::RecentContext;
use crate::sibling_advance;
use crate::theme;
use crate::seek_bar_preview;
use crate::video_pref;
use crate::playback_speed;

/// Application and icon name ([reverse-DNS] for GTK, desktop, and AppStream).
///
/// [reverse-DNS]: https://developer.gnome.org/documentation/tutorials/application-id.html
pub const APP_ID: &str = "ch.rhino.RhinoPlayer";
const APP_WIN_TITLE: &str = "Rhino Player";
/// **Preferences** row for `video_smooth_60`: stores **intent**; the bundled `.vpy` runs only at ~**1.0×**.
const SMOOTH60_MENU_LABEL: &str = "Smooth video (~60 FPS at 1.0×)";
const SEEK_BAR_MENU_LABEL: &str = "Progress bar preview";
const LICENSE_NOTICE: &str = "GPL-3.0-or-later";

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
            eprintln!("[rhino] aspect: sync: non-positive display dims {}×{}", w, h);
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
fn video_pref_submenu_rebuild(
    m: &gio::Menu,
    p: &db::VideoPrefs,
    app: &adw::Application,
) {
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

    let seek_bar_preview = gio::SimpleAction::new_stateful(
        "seek-bar-preview",
        None,
        &seek_bar_on.get().to_variant(),
    );
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
    let show = if recent.is_visible() { true } else { bar_show.get() };
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
                ctx2
                    .squelch
                    .set(Some(Instant::now() + LAYOUT_SQUELCH));
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

/// [video_pref::apply_mpv_video] after [loadfile] so the VapourSynth filter attaches when [path] is valid.
#[derive(Clone)]
struct VideoReapply60 {
    vp: Rc<RefCell<db::VideoPrefs>>,
    app: adw::Application,
}

/// Options for [try_load] (keeps the arity clippy limit without `allow`).
struct LoadOpts {
    record: bool,
    play_on_start: bool,
    /// Filled on success so [maybe_advance_sibling_on_eof] can resolve a path if mpv clears it at idle EOF.
    last_path: Rc<RefCell<Option<PathBuf>>>,
    /// Reveal chrome and (re)start 3s auto-hide; `None` for tests or callers without UI bundle.
    on_start: Option<Rc<dyn Fn()>>,
    /// `Some(w/h)` for [sync_window_aspect_from_mpv] / [apply_window_video_aspect]; cleared with no video.
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Fuzzy subtitle auto-pick + hook after a successful `loadfile`.
    on_loaded: Option<Rc<dyn Fn()>>,
    reapply_60: Option<VideoReapply60>,
}

/// Load a file, hide the recent grid overlay, show video; [LoadOpts::record] appends to recent history.
/// [play_on_start]: clear `pause` so playback runs (watch_later can restore a paused file after load; a
/// short delayed [set_property] catches that). **false** for CLI open-on-launch to respect saved state.
fn try_load(
    path: &Path,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    o: &LoadOpts,
) -> Result<(), String> {
    let play_on_start = o.play_on_start;
    let record = o.record;
    eprintln!(
        "[rhino] try_load: path={} exists={} record={} player_ready={} play={}",
        path.display(),
        path.exists(),
        record,
        player.borrow().is_some(),
        play_on_start
    );
    let mut warm_hit = false;
    {
        let mut g = player.borrow_mut();
        let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
        let prev = local_file_from_mpv(&b.mpv).or_else(|| o.last_path.borrow().clone());
        let already_loaded = recent_layer.is_visible()
            && prev.as_ref().is_some_and(|p| same_open_target(p, path));
        if already_loaded {
            warm_hit = true;
            eprintln!("[rhino] try_load: warm preload hit");
        } else {
            let clear_outgoing_resume = is_done_enough_to_drop_continue(&b.mpv)
                && local_file_from_mpv(&b.mpv).is_some();
            let drop_from_history = prev.as_ref().is_some_and(|p| {
                !same_open_target(p, path) && is_done_enough_to_drop_continue(&b.mpv)
            });
            if let Err(e) = b.load_file_path(path, clear_outgoing_resume) {
                eprintln!("[rhino] try_load: loadfile failed: {e}");
                return Err(e);
            }
            eprintln!("[rhino] try_load: loadfile ok");
            if drop_from_history {
                if let Some(p) = prev {
                    remove_continue_entry(&p);
                }
            }
        }
    }
    if !warm_hit {
        if let Some(r) = o.reapply_60.as_ref() {
            let p = Rc::clone(player);
            let r0 = r.clone();
            let _ = glib::idle_add_local_once(move || {
                if let Some(b) = p.borrow().as_ref() {
                    let a = {
                        let mut g = r0.vp.borrow_mut();
                        video_pref::apply_mpv_video(&b.mpv, &mut g, None)
                    };
                    if a.smooth_auto_off {
                        sync_smooth_60_to_off(&r0.app);
                    }
                }
                let p2 = Rc::clone(&p);
                let r1 = r0.clone();
                let _ = glib::idle_add_local_once(move || {
                    if let Some(b) = p2.borrow().as_ref() {
                        let off = {
                            let mut g = r1.vp.borrow_mut();
                            video_pref::reapply_60_if_still_missing(&b.mpv, &mut g)
                        };
                        if off {
                            sync_smooth_60_to_off(&r1.app);
                        }
                    }
                });
            });
        }
    }
    *o.last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    if record {
        history::record(path);
    }
    let t = title_for_open_path(path);
    win.set_title(Some(t.as_str()));
    recent_layer.set_visible(false);
    // on_start may call apply_chrome, which borrow()s the player; drop the try_load borrow_mut first.
    if let Some(f) = o.on_start.as_ref() {
        f();
    }
    gl.queue_render();
    if play_on_start {
        // Raise the window if the app was in the background (another app focused / minimized).
        win.present();
        if let Some(b) = player.borrow().as_ref() {
            let _ = b.mpv.set_property("pause", false);
        }
        let p2 = Rc::clone(player);
        let _ = glib::source::timeout_add_local(
            std::time::Duration::from_millis(100),
            move || {
                if let Some(b) = p2.borrow().as_ref() {
                    let _ = b.mpv.set_property("pause", false);
                }
                glib::ControlFlow::Break
            },
        );
    }
    if let Some(b) = player.borrow().as_ref() {
        sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    }
    schedule_window_fit_h_video(Rc::clone(player), win.clone());
    if !warm_hit {
        if let Some(f) = o.on_loaded.clone() {
            glib::source::idle_add_local_once(move || f());
        }
    }
    Ok(())
}

fn save_mpv_audio(mpv: &Mpv) {
    let vol = mpv.get_property::<f64>("volume").unwrap_or(100.0);
    let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
    db::save_audio(vol, muted);
}

fn save_mpv_state(mpv: &Mpv, sub: &RefCell<db::SubPrefs>) {
    save_mpv_audio(mpv);
    let mut p = sub.borrow_mut();
    if let Ok(sc) = mpv.get_property::<f64>("sub-scale") {
        if sc.is_finite() {
            p.scale = sc;
        }
    }
    db::save_sub(&p);
}

fn vol_icon(muted: bool, vol: f64) -> &'static str {
    if muted || vol < 0.5 {
        "audio-volume-muted-symbolic"
    } else if vol < 33.0 {
        "audio-volume-low-symbolic"
    } else if vol < 66.0 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

/// Header sound popover: mute icon only (fader next to it shows level).
fn vol_mute_pop_icon(muted: bool) -> &'static str {
    if muted {
        "audio-volume-muted-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

const SIBLING_END_SLACK_SEC: f64 = 1.75;
const SIBLING_POS_STALL_TICKS: u8 = 3;
const SIBLING_POS_EPS: f64 = 0.04;

/// State for `maybe_advance_sibling_on_eof`: one-shot flag and tail stall detection.
struct SiblingEofState {
    done: Cell<bool>,
    stall: Cell<(f64, u8)>,
    /// Last canonical path for which `nav_sensitivity` was computed; avoids `prev` / `next` directory walks every 200ms.
    nav_key: RefCell<Option<PathBuf>>,
    nav_can_prev: Cell<bool>,
    nav_can_next: Cell<bool>,
}

impl SiblingEofState {
    /// Prev/next button sensitivity for `cur`. Reuses cached fs work while the file path is unchanged.
    fn nav_sensitivity(&self, cur: &Path) -> (bool, bool) {
        if !cur.is_file() {
            *self.nav_key.borrow_mut() = None;
            return (false, false);
        }
        let can = match std::fs::canonicalize(cur) {
            Ok(p) => p,
            Err(_) => {
                *self.nav_key.borrow_mut() = None;
                return (false, false);
            }
        };
        {
            let k = self.nav_key.borrow();
            if k.as_ref() == Some(&can) {
                return (self.nav_can_prev.get(), self.nav_can_next.get());
            }
        }
        let cp = sibling_advance::prev_before_current(cur).is_some();
        let cn = sibling_advance::next_after_eof(cur).is_some();
        *self.nav_key.borrow_mut() = Some(can);
        self.nav_can_prev.set(cp);
        self.nav_can_next.set(cn);
        (cp, cn)
    }

    fn clear_nav_sensitivity(&self) {
        *self.nav_key.borrow_mut() = None;
    }
}

/// `eof-reached` is the usual “finished” signal, but with `keep-open` and the GL render path it can stay
/// false while `time-pos` sits just short of `duration` (e.g. one second left) so nothing advances. We also
/// treat as natural end: **unpaused**, within `SIBLING_END_SLACK_SEC` of the end, and the same `time-pos` for
/// `SIBLING_POS_STALL_TICKS` consecutive poll periods (~200 ms each) — playback stuck in the tail.
/// `sibling_eof_done` still allows a single `try_load` per logical end. Clears when not at an end state.
#[allow(clippy::too_many_arguments)]
fn maybe_advance_sibling_on_eof(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    win: &adw::ApplicationWindow,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
    seof: &SiblingEofState,
    on_start: &Rc<dyn Fn()>,
    win_aspect: Rc<Cell<Option<f64>>>,
    on_loaded: Option<Rc<dyn Fn()>>,
    reapply: &VideoReapply60,
) {
    let g = match player.try_borrow() {
        Ok(b) => b,
        Err(_) => return,
    };
    let Some(pl) = g.as_ref() else {
        return;
    };
    let eof = pl.mpv.get_property::<bool>("eof-reached").unwrap_or(false);
    let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
    let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
    let paused = pl.mpv.get_property::<bool>("pause").unwrap_or(true);
    let rem = if dur > 0.0 && pos.is_finite() { dur - pos } else { f64::INFINITY };
    let in_slack = dur > 0.0 && rem <= SIBLING_END_SLACK_SEC;
    if paused || !in_slack || eof {
        seof.stall.set((0.0, 0));
    } else {
        let (lp, n) = seof.stall.get();
        if (pos - lp).abs() < SIBLING_POS_EPS {
            seof.stall.set((lp, n.saturating_add(1).min(250)));
        } else {
            seof.stall.set((pos, 0));
        }
    }
    let stalled = in_slack
        && !paused
        && !eof
        && seof.stall.get().1 >= SIBLING_POS_STALL_TICKS;
    let at_end = eof || stalled;
    if !at_end {
        seof.done.set(false);
        return;
    }
    if seof.done.get() {
        return;
    }
    let finished = local_file_from_mpv(&pl.mpv)
        .or_else(|| last_path.borrow().clone());
    let Some(finished) = finished else {
        seof.done.set(true);
        seof.stall.set((0.0, 0));
        return;
    };
    let next = sibling_advance::next_after_eof(&finished);
    let no_sibling = next.is_none();
    drop(g);
    seof.done.set(true);
    if let Some(np) = next {
        let o = LoadOpts {
            record: true,
            play_on_start: true,
            last_path: Rc::clone(last_path),
            on_start: Some(Rc::clone(on_start)),
            win_aspect: Rc::clone(&win_aspect),
            on_loaded: on_loaded.as_ref().map(Rc::clone),
            reapply_60: Some(reapply.clone()),
        };
        if let Err(e) = try_load(&np, player, win, gl, recent, &o) {
            eprintln!("[rhino] sibling advance: {e}");
            seof.done.set(false);
            seof.stall.set((0.0, 0));
        }
    } else if no_sibling {
        // [try_load] only runs on a path change; with no follow-up file, EOF still left the
        // title in continue + watch_later — drop both here.
        remove_continue_entry(&finished);
    }
}

/// Bottom-bar **Previous** / **Next** tooltips: the **file name** of the target in folder/sibling
/// order; [can] is from [SiblingEofState::nav_sensitivity].
fn sibling_bar_tooltip(is_prev: bool, can: bool, cur: Option<&Path>) -> String {
    if !can {
        return if is_prev {
            "No previous file in folder order".to_string()
        } else {
            "No next file in folder order".to_string()
        };
    }
    let Some(c) = cur else {
        return if is_prev {
            "Open previous in folder order".to_string()
        } else {
            "Open next in folder order".to_string()
        };
    };
    let t = if is_prev {
        sibling_advance::prev_before_current(c)
    } else {
        sibling_advance::next_after_eof(c)
    };
    let Some(t) = t else {
        // Rare if [can] and [cur] match [nav_sensitivity]; keep a neutral line if paths diverge.
        return if is_prev {
            "Previous in folder order".to_string()
        } else {
            "Next in folder order".to_string()
        };
    };
    // File name only (non-utf8: lossy); icon shows previous vs next.
    t.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| t.to_string_lossy().into_owned())
}

fn nudge_mpv_volume(mpv: &Mpv, delta: f64) {
    let max = mpv.get_property::<f64>("volume-max").unwrap_or(100.0).max(1.0);
    let cur = mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let nv = (cur + delta).clamp(0.0, max);
    let _ = mpv.set_property("volume", nv);
    if nv > 0.5 {
        let _ = mpv.set_property("mute", false);
    }
}

/// Rebuild the continue row from [history] after a remove or undo.
fn reflow_continue_cards(
    row: &gtk::Box,
    recent: &gtk::ScrolledWindow,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    let r: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if r.is_empty() {
        recent.set_visible(false);
        return;
    }
    recent.set_visible(true);
    let v: Vec<CardData> = card_data_list(&r);
    recent_view::fill_row(
        row,
        v,
        on_open.clone(),
        on_remove.clone(),
        on_trash.clone(),
    );
    let n = recent_view::ensure_recent_backfill(
        rbf,
        row,
        on_open,
        on_remove,
        on_trash,
    );
    recent_view::schedule_thumb_backfill(n, r);
}

fn cancel_undo_timer(src: &RefCell<Option<glib::source::SourceId>>) {
    if let Some(id) = src.borrow_mut().take() {
        id.remove();
    }
}

/// LIFO stack: label shows the file that **Undo** will restore; dismiss / timeout discards that undo target only.
fn sync_undo_bar(
    label: &gtk::Label,
    btn: &gtk::Button,
    shell: &gtk::Box,
    stack: &RefCell<Vec<ContinueBarUndo>>,
) {
    let n = stack.borrow().len();
    shell.set_visible(n > 0);
    if n == 0 {
        label.set_label("");
        btn.set_tooltip_text(None);
        return;
    }
    match n {
        1 => btn.set_tooltip_text(Some(
            "Undo: put the file back on the list with prior resume/cache, or restore from trash when the last action was trash.",
        )),
        n => {
            let s = format!(
                "Restores the most recent action. {n} step(s) on the stack (one per click, newest first)."
            );
            btn.set_tooltip_text(Some(s.as_str()));
        }
    }
    if let Some(p) = stack.borrow().last() {
        let (name, tail) = match p {
            ContinueBarUndo::ListRemove(u) => (
                u.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "removed from continue list",
            ),
            ContinueBarUndo::Trash { snap, .. } => (
                snap.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file"),
                "moved to trash",
            ),
        };
        let line = format!("\u{201c}{name}\u{201d} {tail}");
        label.set_label(&line);
    }
}

fn rearm_undo_dismiss(
    do_commit: &Rc<dyn Fn() + 'static>,
    undo_source: &RefCell<Option<glib::source::SourceId>>,
) {
    cancel_undo_timer(undo_source);
    let c = do_commit.clone();
    *undo_source.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
        c();
        glib::ControlFlow::Break
    }));
}

/// Shared handles for leaving playback and repainting the recent grid (Escape path).
struct BackToBrowseCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    on_trash: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Show bars; cancel auto-hide. Call after [gtk::ScrolledWindow::set_visible] for the grid.
    on_browse: Rc<dyn Fn()>,
    undo_shell: gtk::Box,
    undo_label: gtk::Label,
    undo_btn: gtk::Button,
    undo_timer: Rc<RefCell<Option<glib::source::SourceId>>>,
    /// Stack of removed/trashed entries, newest at the end; [Undo] pops from the end.
    undo_remove_stack: Rc<RefCell<Vec<ContinueBarUndo>>>,
}

/// Show the sheet immediately; save state and repaint cards after a frame while keeping the
/// current file paused as a warm reopen target when the continue list is non-empty.
fn back_to_browse(
    c: &BackToBrowseCtx,
    win: &impl IsA<gtk::Window>,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    row: &gtk::Box,
    clear_undo: bool,
) {
    cancel_undo_timer(&c.undo_timer);
    if clear_undo {
        *c.undo_remove_stack.borrow_mut() = Vec::new();
        sync_undo_bar(
            &c.undo_label,
            &c.undo_btn,
            &c.undo_shell,
            &c.undo_remove_stack,
        );
    }
    c.win_aspect.set(None);
    *c.last_path.borrow_mut() = None;
    c.sibling_seof.done.set(false);
    c.sibling_seof.stall.set((0.0, 0));
    let paths: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if paths.is_empty() {
        recent.set_visible(false);
    } else {
        recent.set_visible(true);
    }
    (c.on_browse)();
    win.upcast_ref::<gtk::Window>().set_title(Some(APP_WIN_TITLE));
    gl.queue_render();
    // Cut audio right away; `stop` stays in idlers so a last-frame screenshot can run first.
    if let Some(b) = c.player.borrow().as_ref() {
        let _ = b.mpv.set_property("pause", true);
    }

    if paths.is_empty() {
        let p2 = c.player.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(b) = p2.borrow().as_ref() {
                b.snapshot_outgoing_before_leave();
                b.save_playback_state();
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        return;
    }

    // FnOnce chain: `idle_add_local_full` requires FnMut, so the grid refill is scheduled from
    // a one-shot idle (paint can run first at DEFAULT_IDLE priority).
    let p_write = c.player.clone();
    let row2 = row.clone();
    let op2 = c.on_open.clone();
    let osl2 = c.on_remove.clone();
    let otr2 = c.on_trash.clone();
    let paths2 = paths;
    let rbb = c.recent_backfill.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if let Some(b) = p_write.borrow().as_ref() {
            b.snapshot_outgoing_before_leave();
        }
        let rbb2 = rbb.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            recent_view::fill_row(&row2, v, op2.clone(), osl2.clone(), otr2.clone());
            let n = recent_view::ensure_recent_backfill(
                &rbb2,
                &row2,
                op2.clone(),
                osl2.clone(),
                otr2.clone(),
            );
            recent_view::schedule_thumb_backfill(n, paths2.clone());
            glib::ControlFlow::Break
        });
    });
}

/// Enables [gio::SimpleAction] `app.close-video` when the player is ready and the continue grid is hidden.
fn sync_close_video_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    a.set_enabled(player.borrow().is_some() && !recent.is_visible());
}

/// Enables [gio::SimpleAction] `app.move-to-trash` for a local file in playback (not streams / empty path).
fn sync_trash_action(
    a: &gio::SimpleAction,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    recent: &impl IsA<gtk::Widget>,
) {
    let g = player.borrow();
    let Some(b) = g.as_ref() else {
        a.set_enabled(false);
        return;
    };
    let ok = !recent.is_visible()
        && local_file_from_mpv(&b.mpv)
            .is_some_and(|p| p.is_file());
    a.set_enabled(ok);
}

/// Hides the window, then (after GTK can draw the hide) saves watch_later/DB, stops, and quits.
fn schedule_quit_persist(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    sub: &Rc<RefCell<db::SubPrefs>>,
    idle_inhib: &Rc<RefCell<Option<u32>>>,
) {
    win.set_visible(false);
    let p = player.clone();
    let a = app.clone();
    let sp = Rc::clone(sub);
    let ic = Rc::clone(idle_inhib);
    let _ = glib::idle_add_local(move || {
        idle_inhibit::clear(&a, &ic);
        if let Some(b) = p.borrow().as_ref() {
            save_mpv_state(&b.mpv, &sp);
            b.commit_quit();
        }
        a.quit();
        glib::ControlFlow::Break
    });
}

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    file_boot: Rc<RefCell<Option<PathBuf>>>,
    on_open_slot: Rc<RefCell<Option<RcPathFn>>>,
) {
    let sub_pref = Rc::new(RefCell::new(db::load_sub()));
    let video_pref = Rc::new(RefCell::new(db::load_video()));
    let reapply_60 = VideoReapply60 {
        vp: Rc::clone(&video_pref),
        app: app.clone(),
    };

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(APP_WIN_TITLE)
        .icon_name(APP_ID)
        .default_width(WIN_INIT_W)
        .default_height(WIN_INIT_H)
        .css_classes(["rp-win"])
        .build();

    let bar_show = Rc::new(Cell::new(true));
    let nav_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let cur_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let ptr_in_gl = Rc::new(Cell::new(false));
    let motion_squelch = Rc::new(Cell::new(None::<Instant>));
    let last_cap_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_gl_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_path = Rc::new(RefCell::new(None::<PathBuf>));
    let seek_bar_on = Rc::new(Cell::new(db::load_seek_bar_preview()));
    let sibling_seof = Rc::new(SiblingEofState {
        done: Cell::new(false),
        stall: Cell::new((0.0, 0u8)),
        nav_key: RefCell::new(None),
        nav_can_prev: Cell::new(false),
        nav_can_next: Cell::new(false),
    });
    let fs_restore = Rc::new(RefCell::new(None::<(i32, i32)>));
    // Stops `connect_maximized_notify` from re-calling `fullscreen` in the `maximized && !fullscreen`
    // case right after `unfullscreen` (same event tick as leaving fullscreen).
    let skip_max_to_fs = Rc::new(Cell::new(false));
    let last_unmax = Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H)));
    let win_aspect = Rc::new(Cell::new(None::<f64>));
    let aspect_resize_end_deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let aspect_resize_wired = Rc::new(Cell::new(false));
    let idle_inhib = Rc::new(RefCell::new(None::<u32>));

    let root = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    let play_pause = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_pause.add_css_class("flat");
    play_pause.add_css_class("rpb-play");
    play_pause.set_tooltip_text(Some("Play (Space)"));
    play_pause.set_sensitive(false);
    let btn_prev = gtk::Button::from_icon_name("go-previous-symbolic");
    btn_prev.add_css_class("flat");
    btn_prev.add_css_class("rpb-prev");
    btn_prev.set_sensitive(false);
    let wrap_prev = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap_prev.append(&btn_prev);
    wrap_prev.set_tooltip_text(Some("Previous file in folder"));
    btn_prev.set_has_tooltip(false);
    let btn_next = gtk::Button::from_icon_name("go-next-symbolic");
    btn_next.add_css_class("flat");
    btn_next.add_css_class("rpb-next");
    btn_next.set_sensitive(false);
    let wrap_next = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    wrap_next.append(&btn_next);
    wrap_next.set_tooltip_text(Some("Next file in folder"));
    btn_next.set_has_tooltip(false);
    let pref_menu = gio::Menu::new();
    pref_menu.append(Some(SMOOTH60_MENU_LABEL), Some("app.smooth-60"));
    pref_menu.append(
        Some("Choose VapourSynth script (.vpy)…"),
        Some("app.choose-vs"),
    );

    let menu = gio::Menu::new();
    menu.append(Some("Open video…"), Some("app.open"));
    menu.append(Some("Close video"), Some("app.close-video"));
    menu.append(Some("Move to Trash"), Some("app.move-to-trash"));
    menu.append_submenu(Some("Preferences"), &pref_menu);
    menu.append(Some("About Rhino Player"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let vol_adj = gtk::Adjustment::new(100.0, 0.0, 100.0, 1.0, 5.0, 0.0);
    let vol_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&vol_adj));
    vol_scale.set_draw_value(false);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(240, -1);
    vol_scale.set_valign(gtk::Align::Center);
    vol_scale.set_tooltip_text(Some("Volume"));
    vol_scale.add_css_class("rp-vol");
    let vol_mute_btn = gtk::ToggleButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .valign(gtk::Align::Center)
        .vexpand(false)
        .tooltip_text("Mute")
        .build();
    vol_mute_btn.add_css_class("flat");
    vol_mute_btn.add_css_class("circular");
    let vol_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    vol_row.set_valign(gtk::Align::Center);
    vol_row.set_size_request(300, -1);
    vol_row.append(&vol_mute_btn);
    vol_row.append(&vol_scale);

    let audio_tracks_block = Rc::new(Cell::new(false));
    let audio_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    audio_tracks_box.set_margin_top(2);
    let audio_tracks_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(400)
        .max_content_height(480)
        .child(&audio_tracks_box)
        .build();
    let audio_tracks_section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    audio_tracks_section.append(&audio_tracks_scrl);
    audio_tracks_section.set_visible(false);
    let sound_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    sound_col.add_css_class("rp-popover-box");
    sound_col.append(&vol_row);
    sound_col.append(&audio_tracks_section);
    let vol_pop = gtk::Popover::new();
    vol_pop.add_css_class("rp-header-popover");
    vol_pop.set_child(Some(&sound_col));
    header_popover_non_modal(&vol_pop);
    let vol_menu = gtk::MenuButton::new();
    vol_menu.set_icon_name("audio-volume-high-symbolic");
    vol_menu.set_tooltip_text(Some("Volume and mute; audio track list if several tracks"));
    vol_menu.set_popover(Some(&vol_pop));
    vol_menu.add_css_class("flat");

    let sp_init = sub_pref.borrow().clone();
    let sub_tracks_block = Rc::new(Cell::new(false));
    let sub_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    sub_tracks_box.set_margin_top(2);
    let sub_tracks_scrl = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .min_content_width(360)
        .max_content_height(280)
        .child(&sub_tracks_box)
        .build();
    let sub_tracks_section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sub_tracks_section.append(&sub_tracks_scrl);
    sub_tracks_section.set_visible(false);

    let sub_scale_adj = gtk::Adjustment::new(sp_init.scale, 0.3, 2.0, 0.05, 0.1, 0.0);
    let sub_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&sub_scale_adj));
    sub_scale.set_draw_value(true);
    sub_scale.set_digits(2);
    sub_scale.set_hexpand(true);
    sub_scale.set_size_request(240, -1);
    sub_scale.set_tooltip_text(Some("Subtitle size (mpv sub-scale)"));

    let sub_color_btn = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    sub_color_btn.set_rgba(&sub_prefs::u32_to_rgba(sp_init.color));
    sub_color_btn.set_tooltip_text(Some("Subtitle text color"));

    let sub_opts = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let sub_size_label = gtk::Label::new(Some("Size"));
    sub_size_label.set_xalign(0.0);
    sub_size_label.add_css_class("caption");
    sub_opts.append(&sub_size_label);
    sub_opts.append(&sub_scale);
    let sub_color_label = gtk::Label::new(Some("Text color"));
    sub_color_label.set_xalign(0.0);
    sub_color_label.add_css_class("caption");
    sub_opts.append(&sub_color_label);
    sub_opts.append(&sub_color_btn);

    let sub_col = gtk::Box::new(gtk::Orientation::Vertical, 10);
    sub_col.add_css_class("rp-popover-box");
    sub_col.append(&sub_tracks_section);
    sub_col.append(&sub_opts);

    let sub_pop = gtk::Popover::new();
    sub_pop.add_css_class("rp-header-popover");
    sub_pop.set_child(Some(&sub_col));
    header_popover_non_modal(&sub_pop);
    let sub_menu = gtk::MenuButton::new();
    sub_menu.set_icon_name("media-view-subtitles-symbolic");
    sub_menu.set_tooltip_text(Some("Subtitles: tracks and style"));
    sub_menu.set_popover(Some(&sub_pop));
    sub_menu.add_css_class("flat");
    sub_menu.set_visible(false);

    let speed_list = gtk::ListBox::new();
    speed_list.set_activate_on_single_click(true);
    speed_list.add_css_class("rich-list");
    for s in &["1.0×", "1.5×", "2.0×"] {
        let row = gtk::ListBoxRow::new();
        let lab = gtk::Label::new(Some(*s));
        lab.set_halign(gtk::Align::Start);
        lab.set_margin_start(10);
        lab.set_margin_end(10);
        lab.set_margin_top(6);
        lab.set_margin_bottom(6);
        row.set_child(Some(&lab));
        speed_list.append(&row);
    }
    let speed_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    speed_col.add_css_class("rp-popover-box");
    speed_col.append(&speed_list);
    let speed_pop = gtk::Popover::new();
    speed_pop.add_css_class("rp-header-popover");
    speed_pop.set_child(Some(&speed_col));
    header_popover_non_modal(&speed_pop);
    let speed_mbtn = gtk::MenuButton::new();
    speed_mbtn.set_icon_name("speedometer-symbolic");
    speed_mbtn.set_tooltip_text(Some("Playback speed"));
    speed_mbtn.set_popover(Some(&speed_pop));
    speed_mbtn.set_sensitive(false);
    speed_mbtn.add_css_class("flat");

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some("Main menu"));
    menu_btn.set_menu_model(Some(&menu));
    {
        let mb = menu_btn.clone();
        menu_btn.connect_notify_local(Some("popover"), move |b, _| {
            if let Some(p) = b.popover() {
                header_popover_non_modal(&p);
            }
        });
        menu_btn.connect_active_notify(move |b| {
            if b.is_active() {
                if let Some(p) = b.popover() {
                    header_popover_non_modal(&p);
                }
            }
        });
        if let Some(p) = mb.popover() {
            header_popover_non_modal(&p);
        }
    }
    header.pack_end(&menu_btn);
    header.pack_end(&vol_menu);
    header.pack_end(&sub_menu);
    header.pack_end(&speed_mbtn);
    header_menubtns_switch([speed_mbtn.clone(), sub_menu.clone(), vol_menu.clone(), menu_btn.clone()]);

    let gl_area = gtk::GLArea::new();
    {
        let p = player.clone();
        let bx = audio_tracks_box.clone();
        let blk = Rc::clone(&audio_tracks_block);
        let gla = gl_area.clone();
        let sec = audio_tracks_section.clone();
        vol_pop.connect_show(move |_| {
            let show = audio_tracks::rebuild_popover(&p, &bx, &blk, &gla);
            sec.set_visible(show);
        });
    }
    {
        let p = player.clone();
        let sp_pick = sub_pref.clone();
        let sp_off = sub_pref.clone();
        let bx = sub_tracks_box.clone();
        let blk = Rc::clone(&sub_tracks_block);
        let gla = gl_area.clone();
        let sec = sub_tracks_section.clone();
        let on_sub_pick: Rc<dyn Fn(&str)> = Rc::new(move |label: &str| {
            {
                let mut s = sp_pick.borrow_mut();
                s.last_sub_label = label.to_string();
                s.sub_off = false;
            }
            db::save_sub(&sp_pick.borrow());
        });
        let on_sub_off: Rc<dyn Fn()> = Rc::new(move || {
            sp_off.borrow_mut().sub_off = true;
            db::save_sub(&sp_off.borrow());
        });
        sub_pop.connect_show(move |_| {
            let show = sub_tracks::rebuild_popover(
                &p,
                &bx,
                &blk,
                &gla,
                Some(Rc::clone(&on_sub_pick)),
                Some(Rc::clone(&on_sub_off)),
            );
            sec.set_visible(show);
        });
    }
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    {
        let p_btn = player.clone();
        let glbtn = gl_area.clone();
        play_pause.connect_clicked(move |_| {
            let g = p_btn.borrow();
            let Some(b) = g.as_ref() else {
                return;
            };
            if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
                return;
            }
            let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
            if b.mpv.set_property("pause", !paused).is_err() {
                return;
            }
            glbtn.queue_render();
        });
    }

    let rpp = gtk::GestureClick::new();
    rpp.set_button(gtk::gdk::BUTTON_SECONDARY);
    rpp.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let p_btn = player.clone();
        let glbtn = gl_area.clone();
        rpp.connect_pressed(move |gest, n_press, _, _| {
            // Stops the compositor / shell default (e.g. window context menu) on the video surface.
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            if n_press != 1 {
                return;
            }
            let g = p_btn.borrow();
            let Some(b) = g.as_ref() else {
                return;
            };
            if b.mpv.get_property::<f64>("duration").unwrap_or(0.0) <= 0.0 {
                return;
            }
            let paused = b.mpv.get_property::<bool>("pause").unwrap_or(false);
            if b.mpv.set_property("pause", !paused).is_err() {
                return;
            }
            glbtn.queue_render();
        });
    }
    gl_area.add_controller(rpp);

    let seek_adj = gtk::Adjustment::new(0.0, 0.0, 1.0, 0.2, 1.0, 0.0);
    let seek = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&seek_adj));
    seek.set_hexpand(true);
    seek.set_draw_value(false);
    seek.set_sensitive(false);
    seek.add_css_class("rp-seek");
    seek.set_size_request(120, 0);
    let time_left = gtk::Label::new(Some("0:00"));
    time_left.add_css_class("rp-time");
    time_left.set_xalign(0.0);
    let time_right = gtk::Label::new(Some("0:00"));
    time_right.set_css_classes(&["rp-time", "rp-time-dim"]);
    time_right.set_xalign(1.0);

    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bottom.add_css_class("rp-bottom");
    bottom.set_vexpand(false);
    play_pause.set_valign(gtk::Align::Center);
    wrap_prev.set_valign(gtk::Align::Center);
    wrap_next.set_valign(gtk::Align::Center);
    bottom.append(&wrap_prev);
    bottom.append(&play_pause);
    bottom.append(&wrap_next);
    let speed_sync = Rc::new(Cell::new(false));
    let vp_speed = Rc::clone(&video_pref);
    let app_speed = app.clone();
    {
        let p = player.clone();
        let glr = gl_area.clone();
        let sy = speed_sync.clone();
        let smb = speed_mbtn.clone();
        let vp = Rc::clone(&vp_speed);
        let ap = app_speed.clone();
        speed_list.connect_row_activated(move |list2, row| {
            if sy.get() {
                return;
            }
            let i: u32 = (0i32..3)
                .find(|&ix| {
                    list2
                        .row_at_index(ix)
                        .is_some_and(|r| r == *row)
                })
                .unwrap_or(0) as u32;
            let v = playback_speed::value_at(i);
            if let Some(b) = p.borrow().as_ref() {
                let _ = b.mpv.set_property("speed", v);
                glr.queue_render();
                // Defer [vf] rebuild: libmpv can still report the old [speed] on the same GTK tick as
                // [set_property]; [mvtools_vf_eligible] + [add_smooth_60] must see 1.0× when returning from 1.5/2.0.
                let bref = p.clone();
                let vp2 = Rc::clone(&vp);
                let ap2 = ap.clone();
                let vh = v;
                let _ = glib::idle_add_local_once(move || {
                    if let Some(pl) = bref.borrow().as_ref() {
                        let mut g = vp2.borrow_mut();
                        if video_pref::refresh_smooth_for_playback_speed(&pl.mpv, &mut g, Some(vh)) {
                            sync_smooth_60_to_off(&ap2);
                        }
                    }
                });
            }
            smb.set_active(false);
        });
    }
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);
    {
        let b = gtk::Button::from_icon_name("window-close-symbolic");
        b.set_tooltip_text(Some("Close video (Ctrl+W)"));
        b.add_css_class("flat");
        b.set_valign(gtk::Align::Center);
        b.set_action_name(Some("app.close-video"));
        b.set_margin_start(4);
        bottom.append(&b);
    }

    let seek_state = seek_bar_preview::connect(
        &seek,
        &seek_adj,
        Rc::clone(player),
        Rc::clone(&last_path),
        Rc::clone(&seek_bar_on),
    );

    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(&gl_area));

    let (recent_scrl, flow_recent, sp_empty, undo_bar) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);
    let undo_shell = undo_bar.shell.clone();
    let undo_label = undo_bar.label.clone();
    let undo_btn = undo_bar.undo.clone();

    let close_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));
    let trash_act_for_sync: Rc<RefCell<Option<gio::SimpleAction>>> = Rc::new(RefCell::new(None));

    let on_file_loaded: Rc<dyn Fn()> = Rc::new({
        let p = player.clone();
        let sp = sub_pref.clone();
        let g2 = gl_area.clone();
        let bshow = bar_show.clone();
        let rec = recent_scrl.clone();
        let bot = bottom.clone();
        let sub_m_btn = sub_menu.clone();
        let close_a = Rc::clone(&close_act_for_sync);
        let trash_a = Rc::clone(&trash_act_for_sync);
        let syf = speed_sync.clone();
        let sl = speed_list.clone();
        let vp_onload = Rc::clone(&video_pref);
        let app_onload = app.clone();
        move || {
            let p2 = p.clone();
            let sp2 = sp.clone();
            let g3 = g2.clone();
            let b3 = bshow.clone();
            let r3 = rec.clone();
            let bot2 = bot.clone();
            let sub320 = sub_m_btn.clone();
            let close_a2 = Rc::clone(&close_a);
            let trash_a2 = Rc::clone(&trash_a);
            let syf320 = syf.clone();
            let sl320 = sl.clone();
            let vp_320 = Rc::clone(&vp_onload);
            let app_320 = app_onload.clone();
            let _ = glib::timeout_add_local(Duration::from_millis(320), move || {
                if let Some(b) = p2.borrow().as_ref() {
                    sub320.set_visible(sub_tracks::has_subtitle_tracks(&b.mpv));
                    let pr = sp2.borrow();
                    sub_prefs::apply_mpv(&b.mpv, &pr);
                    let show = if r3.is_visible() { true } else { b3.get() };
                    sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot2.height(), g3.height());
                    audio_tracks::restore_saved_audio(&b.mpv);
                    audio_tracks::ensure_playable_audio(&b.mpv);
                    sub_tracks::autopick_sub_track(&b.mpv, &pr);
                    let listed = playback_speed::sync_list(&b.mpv, &syf320, &sl320);
                    let mut g = vp_320.borrow_mut();
                    if g.smooth_60 {
                        let off = if let Some(s) = listed {
                            video_pref::refresh_smooth_for_playback_speed(&b.mpv, &mut g, Some(s))
                        } else if video_pref::needs_playback_speed_env_resync(&b.mpv) {
                            video_pref::refresh_smooth_for_playback_speed(&b.mpv, &mut g, None)
                        } else {
                            video_pref::resync_smooth_if_speed_mismatch(&b.mpv, &mut g)
                        };
                        if off {
                            sync_smooth_60_to_off(&app_320);
                        }
                    }
                }
                if let Some(a) = close_a2.borrow().as_ref() {
                    sync_close_video_action(a, &p2, &r3);
                }
                if let Some(a) = trash_a2.borrow().as_ref() {
                    sync_trash_action(a, &p2, &r3);
                }
                glib::ControlFlow::Break
            });
            // 60p: [try_load] chains a second idle to [reapply_60_if_still_missing]. This 320ms hook
            // catches watch-later [speed] / list snap and [vf] vs [mvtools_vf_eligible] in one pass.
        }
    });
    {
        let p = player.clone();
        let sp = sub_pref.clone();
        let gll = gl_area.clone();
        let adj = sub_scale_adj.clone();
        let bshow = bar_show.clone();
        let rec = recent_scrl.clone();
        let bot = bottom.clone();
        sub_scale_adj.connect_value_changed(move |_| {
            let v = adj.value();
            sp.borrow_mut().scale = v;
            if let Some(b) = p.borrow().as_ref() {
                let pr = sp.borrow();
                sub_prefs::apply_mpv(&b.mpv, &pr);
                let show = if rec.is_visible() { true } else { bshow.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot.height(), gll.height());
            }
            db::save_sub(&sp.borrow());
            gll.queue_render();
        });
    }
    {
        let p = player.clone();
        let sp = sub_pref.clone();
        let gll = gl_area.clone();
        let btn = sub_color_btn.clone();
        let bshow = bar_show.clone();
        let rec = recent_scrl.clone();
        let bot = bottom.clone();
        sub_color_btn.connect_rgba_notify(move |_| {
            sp.borrow_mut().color = sub_prefs::rgba_to_u32(&btn.rgba());
            if let Some(b) = p.borrow().as_ref() {
                let pr = sp.borrow();
                sub_prefs::apply_mpv(&b.mpv, &pr);
                let show = if rec.is_visible() { true } else { bshow.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, bot.height(), gll.height());
            }
            db::save_sub(&sp.borrow());
            gll.queue_render();
        });
    }

    // Double-tap fullscreen on the video (GLArea = hit target). Use **connect_pressed** and
    // `n_press == 2` on the *second* press (same as pre–skip/notify refactors) — on some stacks
    // `connect_released` does not report `n_press == 2` reliably for leaving fullscreen.
    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    {
        let win_fs = win.clone();
        let fr = fs_restore.clone();
        let lu = last_unmax.clone();
        let skip_dbl = skip_max_to_fs.clone();
        let rec_dbl = recent_scrl.clone();
        dbl.connect_pressed(move |gest, n_press, _, _| {
            if n_press != 2 {
                return;
            }
            if rec_dbl.is_visible() {
                return;
            }
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            toggle_fullscreen(&win_fs, &fr, &lu, &skip_dbl);
        });
    }
    gl_area.add_controller(dbl);

    for sp in sp_empty {
        let d2 = gtk::GestureClick::new();
        d2.set_button(gtk::gdk::BUTTON_PRIMARY);
        let w2 = win.clone();
        let fr2 = fs_restore.clone();
        let lu2 = last_unmax.clone();
        let sk2 = skip_max_to_fs.clone();
        let rec2 = recent_scrl.clone();
        d2.connect_pressed(move |gest, n_press, _, _| {
            if n_press != 2 {
                return;
            }
            if !rec2.is_visible() {
                return;
            }
            let _ = gest.set_state(gtk::EventSequenceState::Claimed);
            toggle_fullscreen(&w2, &fr2, &lu2, &sk2);
        });
        sp.add_controller(d2);
    }

    let want_recent = file_boot.borrow().is_none() && !history::load().is_empty();
    recent_scrl.set_visible(want_recent);

    let ch_hide = Rc::new(ChromeBarHide {
        nav: nav_t.clone(),
        vol: vol_menu.clone(),
        sub: sub_menu.clone(),
        speed: speed_mbtn.clone(),
        main: menu_btn.clone(),
        root: root.clone(),
        gl: gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: recent_scrl.clone(),
        bottom: bottom.clone(),
        player: player.clone(),
        squelch: motion_squelch.clone(),
    });

    let on_video_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let bot = bottom.clone();
        let p = player.clone();
        let chh = Rc::clone(&ch_hide);
        Rc::new(move || {
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
            schedule_bars_autohide(Rc::clone(&chh));
        })
    };
    {
        let ch = Rc::clone(&ch_hide);
        let h = Rc::new(move || {
            let any = ch.vol.is_active()
                || ch.sub.is_active()
                || ch.speed.is_active()
                || ch.main.is_active();
            if any {
                if let Some(id) = ch.nav.borrow_mut().take() {
                    id.remove();
                }
                ch.bar_show.set(true);
                apply_chrome(
                    &ch.root,
                    &ch.gl,
                    &ch.bar_show,
                    &ch.recent,
                    &ch.bottom,
                    &ch.player,
                );
            } else {
                schedule_bars_autohide(Rc::clone(&ch));
            }
        });
        let h1 = Rc::clone(&h);
        let h2 = Rc::clone(&h);
        let h3 = Rc::clone(&h);
        let h4 = Rc::clone(&h);
        vol_menu.connect_active_notify(move |_| h1());
        sub_menu.connect_active_notify(move |_| h3());
        speed_mbtn.connect_active_notify(move |_| h4());
        menu_btn.connect_active_notify(move |_| h2());
    }
    let browse_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let bot = bottom.clone();
        let p = player.clone();
        let nav = nav_t.clone();
        Rc::new(move || {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent, &bot, &p);
        })
    };
    let on_open_vid = on_video_chrome.clone();
    let on_start_menu = on_open_vid.clone();
    let ol_open = Rc::clone(&on_file_loaded);
    let p_openr = player.clone();
    let win_menu = win.clone();
    let gl_op = gl_area.clone();
    let recent_on_top = recent_scrl.clone();
    let last_open = last_path.clone();
    let wa_on = Rc::clone(&win_aspect);
    let reapply_on_open = reapply_60.clone();
    let on_open: RcPathFn = Rc::new(move |path: &Path| {
        eprintln!("[rhino] on_open from recent/menu: {}", path.display());
        if let Err(e) = try_load(
            path,
            &p_openr,
            &win_menu,
            &gl_op,
            &recent_on_top,
            &LoadOpts {
                record: true,
                play_on_start: true,
                last_path: last_open.clone(),
                on_start: Some(Rc::clone(&on_start_menu)),
                win_aspect: wa_on.clone(),
                on_loaded: Some(Rc::clone(&ol_open)),
                reapply_60: Some(reapply_on_open.clone()),
            },
        ) {
            eprintln!("[rhino] on_open: try_load error: {e}");
        }
    });
    *on_open_slot.borrow_mut() = Some(on_open.clone());

    {
        let p = player.clone();
        let w = win.clone();
        let gla = gl_area.clone();
        let rec = recent_scrl.clone();
        let lp = last_path.clone();
        let ovid = on_open_vid.clone();
        let wa = win_aspect.clone();
        let seof = sibling_seof.clone();
        let ol = Rc::clone(&on_file_loaded);
        btn_prev.connect_clicked(glib::clone!(
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            gla,
            #[strong]
            rec,
            #[strong]
            lp,
            #[strong]
            ovid,
            #[strong]
            wa,
            #[strong]
            seof,
            #[strong]
            ol,
            #[strong]
            reapply_60,
            move |_| {
                let g = p.borrow();
                let Some(pl) = g.as_ref() else {
                    return;
                };
                let cur = local_file_from_mpv(&pl.mpv).or_else(|| lp.borrow().clone());
                let Some(cur) = cur.filter(|c| c.is_file()) else {
                    return;
                };
                let Some(np) = sibling_advance::prev_before_current(&cur) else {
                    return;
                };
                seof.done.set(false);
                seof.stall.set((0.0, 0));
                drop(g);
                let o = LoadOpts {
                    record: true,
                    play_on_start: true,
                    last_path: Rc::clone(&lp),
                    on_start: Some(Rc::clone(&ovid)),
                    win_aspect: Rc::clone(&wa),
                    on_loaded: Some(Rc::clone(&ol)),
                    reapply_60: Some(reapply_60.clone()),
                };
                if let Err(e) = try_load(&np, &p, &w, &gla, &rec, &o) {
                    eprintln!("[rhino] previous: {e}");
                }
            }
        ));
        let ol2 = Rc::clone(&on_file_loaded);
        btn_next.connect_clicked(glib::clone!(
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            gla,
            #[strong]
            rec,
            #[strong]
            lp,
            #[strong]
            ovid,
            #[strong]
            wa,
            #[strong]
            seof,
            #[strong]
            ol2,
            #[strong]
            reapply_60,
            move |_| {
                let g = p.borrow();
                let Some(pl) = g.as_ref() else {
                    return;
                };
                let cur = local_file_from_mpv(&pl.mpv).or_else(|| lp.borrow().clone());
                let Some(cur) = cur.filter(|c| c.is_file()) else {
                    return;
                };
                let Some(np) = sibling_advance::next_after_eof(&cur) else {
                    return;
                };
                seof.done.set(false);
                seof.stall.set((0.0, 0));
                drop(g);
                let o = LoadOpts {
                    record: true,
                    play_on_start: true,
                    last_path: Rc::clone(&lp),
                    on_start: Some(Rc::clone(&ovid)),
                    win_aspect: Rc::clone(&wa),
                    on_loaded: Some(Rc::clone(&ol2)),
                    reapply_60: Some(reapply_60.clone()),
                };
                if let Err(e) = try_load(&np, &p, &w, &gla, &rec, &o) {
                    eprintln!("[rhino] next: {e}");
                }
            }
        ));
    }

    let recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>> = Rc::new(RefCell::new(None));
    let pending_recent_backfill: Rc<RefCell<Option<RecentBackfillJob>>> =
        Rc::new(RefCell::new(None));
    let recent_backfill_start: Rc<dyn Fn(Rc<RecentContext>, Vec<PathBuf>)> = {
        let p = player.clone();
        let pending = pending_recent_backfill.clone();
        Rc::new(move |ctx, paths| schedule_or_defer_recent_backfill(&p, &pending, ctx, paths))
    };
    {
        let rb = recent_backfill.clone();
        let pending = pending_recent_backfill.clone();
        recent_scrl.connect_destroy(move |_| {
            pending.borrow_mut().take();
            if let Some(ctx) = rb.borrow_mut().take() {
                ctx.shutdown();
            }
        });
    }

    let undo_remove_stack = Rc::new(RefCell::new(Vec::<ContinueBarUndo>::new()));
    let undo_timer = Rc::new(RefCell::new(None::<glib::source::SourceId>));
    type DismissTopRef = Rc<RefCell<Option<Weak<dyn Fn() + 'static>>>>;
    let do_commit_weak: DismissTopRef = Rc::new(RefCell::new(None));
    let ush_d = undo_shell.clone();
    let ul_d = undo_label.clone();
    let ub_d = undo_btn.clone();
    let urs_d = undo_remove_stack.clone();
    let uts_d = undo_timer.clone();
    let wk_d = do_commit_weak.clone();
    let do_commit: Rc<dyn Fn() + 'static> = Rc::new(move || {
        cancel_undo_timer(uts_d.as_ref());
        if urs_d.borrow_mut().pop().is_none() {
            return;
        }
        sync_undo_bar(&ul_d, &ub_d, &ush_d, &urs_d);
        if !urs_d.borrow().is_empty() {
            if let Some(f) = wk_d
                .borrow()
                .as_ref()
                .and_then(|w| w.upgrade())
            {
                *uts_d.borrow_mut() = Some(glib::timeout_add_seconds_local(10, move || {
                    f();
                    glib::ControlFlow::Break
                }));
            }
        }
    });
    *do_commit_weak.borrow_mut() = Some(Rc::downgrade(&do_commit));
    let on_remove_cell: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    let on_trash_slot: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    let fr_sl = flow_recent.clone();
    let recent_rm = recent_scrl.clone();
    let op_s = on_open.clone();
    let rbf_rm = recent_backfill.clone();
    let ur_stack = undo_remove_stack.clone();
    let u_sh_rm = undo_shell.clone();
    let undo_t_rm = undo_btn.clone();
    let u_la_rm = undo_label.clone();
    let ut_rm = undo_timer.clone();
    let do_rm = do_commit.clone();
    let cell_rm = on_remove_cell.clone();
    let cell_t = on_trash_slot.clone();
    let on_trash: RcPathFn = Rc::new({
        let fr_t = fr_sl.clone();
        let rec_t = recent_rm.clone();
        let op_t = op_s.clone();
        let rbf_t = rbf_rm.clone();
        let ur_t = ur_stack.clone();
        let u_la_t = u_la_rm.clone();
        let undo_t_t = undo_t_rm.clone();
        let u_sh_t = u_sh_rm.clone();
        let do_t = do_rm.clone();
        let ut_t = ut_rm.clone();
        let cell_rm = cell_rm.clone();
        let cell_t = cell_t.clone();
        move |path: &Path| {
            if !path.is_file() {
                return;
            }
            let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            let snap = capture_list_remove_undo(path);
            if let Err(e) = gio::File::for_path(path).trash(gio::Cancellable::NONE) {
                eprintln!("[rhino] move to trash (continue card): {e}");
                return;
            }
            let in_trash = trash_xdg::find_trash_files_stored_path(&want);
            if in_trash.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            remove_continue_entry(path);
            if let Some(t) = in_trash {
                ur_t.borrow_mut().push(ContinueBarUndo::Trash { snap, in_trash: t });
                sync_undo_bar(&u_la_t, &undo_t_t, &u_sh_t, &ur_t);
                rearm_undo_dismiss(&do_t, ut_t.as_ref());
            }
            let f = cell_rm
                .borrow()
                .as_ref()
                .expect("on_remove not wired")
                .clone();
            let t = cell_t
                .borrow()
                .as_ref()
                .expect("on_trash not wired")
                .clone();
            reflow_continue_cards(&fr_t, &rec_t, op_t.clone(), f, t, &rbf_t);
        }
    });
    *on_trash_slot.borrow_mut() = Some(on_trash.clone());
    let on_remove: RcPathFn = Rc::new({
        let cell_rm = on_remove_cell.clone();
        let tslot = on_trash_slot.clone();
        let fr_sl = fr_sl;
        let recent_rm = recent_rm;
        let op_s = op_s;
        let rbf_rm = rbf_rm;
        let ur_stack = ur_stack.clone();
        let u_la_rm = u_la_rm.clone();
        let undo_t_rm = undo_t_rm.clone();
        let u_sh_rm = u_sh_rm.clone();
        let do_rm = do_rm.clone();
        let ut_rm = ut_rm.clone();
        move |path: &Path| {
            let u = capture_list_remove_undo(path);
            remove_continue_entry(path);
            ur_stack
                .borrow_mut()
                .push(ContinueBarUndo::ListRemove(u));
            sync_undo_bar(
                &u_la_rm,
                &undo_t_rm,
                &u_sh_rm,
                &ur_stack,
            );
            let f = cell_rm
                .borrow()
                .as_ref()
                .expect("on_remove not wired")
                .clone();
            let t = tslot
                .borrow()
                .as_ref()
                .expect("on_trash not wired")
                .clone();
            reflow_continue_cards(&fr_sl, &recent_rm, op_s.clone(), f, t, &rbf_rm);
            rearm_undo_dismiss(&do_rm, ut_rm.as_ref());
        }
    });
    *on_remove_cell.borrow_mut() = Some(on_remove.clone());

    {
        let fr_u = flow_recent.clone();
        let rec_u = recent_scrl.clone();
        let op_u = on_open.clone();
        let rbf_u = recent_backfill.clone();
        let ur_u = undo_remove_stack.clone();
        let u_sh_u = undo_shell.clone();
        let undo_t_u = undo_btn.clone();
        let u_la_u = undo_label.clone();
        let ut_u = undo_timer.clone();
        let do_u = do_commit.clone();
        let cell_u = on_remove_cell.clone();
        let tslot_u = on_trash_slot.clone();
        undo_btn.connect_clicked(glib::clone!(
            #[strong]
            fr_u,
            #[strong]
            rec_u,
            #[strong]
            op_u,
            #[strong]
            rbf_u,
            #[strong]
            ur_u,
            #[strong]
            u_sh_u,
            #[strong]
            undo_t_u,
            #[strong]
            u_la_u,
            #[strong]
            ut_u,
            #[strong]
            do_u,
            #[strong]
            cell_u,
            #[strong]
            tslot_u,
            move |_| {
                cancel_undo_timer(ut_u.as_ref());
                let Some(undo) = ur_u.borrow_mut().pop() else {
                    return;
                };
                if let Err(e) = apply_bar_undo(&undo) {
                    eprintln!("[rhino] undo: {e}");
                    ur_u.borrow_mut().push(undo);
                    return;
                }
                history::record(undo.target_path());
                sync_undo_bar(&u_la_u, &undo_t_u, &u_sh_u, &ur_u);
                rec_u.set_visible(true);
                let f = cell_u
                    .borrow()
                    .as_ref()
                    .expect("on_remove not wired")
                    .clone();
                let t = tslot_u
                    .borrow()
                    .as_ref()
                    .expect("on_trash not wired")
                    .clone();
                reflow_continue_cards(&fr_u, &rec_u, op_u.clone(), f, t, &rbf_u);
                if !ur_u.borrow().is_empty() {
                    rearm_undo_dismiss(&do_u, ut_u.as_ref());
                }
            }
        ));
    }
    {
        let dc = do_commit.clone();
        undo_bar.close.connect_clicked(move |_| {
            dc();
        });
    }

    if want_recent {
        let paths5: Vec<PathBuf> = history::load().into_iter().take(5).collect();
        recent_view::fill_idle(
            &flow_recent,
            paths5,
            on_open.clone(),
            on_remove.clone(),
            on_trash.clone(),
            recent_backfill.clone(),
            recent_backfill_start.clone(),
        );
    }

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

    let close_video = gio::SimpleAction::new("close-video", None);
    let p_btv = player.clone();
    let w_btv = win.clone();
    let recent_btv = recent_scrl.clone();
    let flow_btv = flow_recent.clone();
    let gl_btv = gl_area.clone();
    let op_btv = on_open.clone();
    let rem_btv = on_remove.clone();
    let trash_btv = on_trash.clone();
    let rbf_btv = recent_backfill.clone();
    let last_btv = last_path.clone();
    let seof_btv = sibling_seof.clone();
    let browse_btv = browse_chrome.clone();
    let wa_btv = win_aspect.clone();
    let ush_btv = undo_shell.clone();
    let ula_btv = undo_label.clone();
    let uti_btv = undo_timer.clone();
    let ur_btv = undo_remove_stack.clone();
    let undo_t_btv = undo_btn.clone();
    close_video.connect_activate(glib::clone!(
        #[strong]
        p_btv,
        #[strong]
        w_btv,
        #[strong]
        recent_btv,
        #[strong]
        flow_btv,
        #[strong]
        gl_btv,
        #[strong]
        op_btv,
        #[strong]
        rem_btv,
        #[strong]
        trash_btv,
        #[strong]
        rbf_btv,
        #[strong]
        last_btv,
        #[strong]
        seof_btv,
        #[strong]
        browse_btv,
        #[strong]
        wa_btv,
        #[strong]
        ush_btv,
        #[strong]
        ula_btv,
        #[strong]
        uti_btv,
        #[strong]
        ur_btv,
        #[strong]
        undo_t_btv,
        move |_, _| {
            if recent_btv.is_visible() || p_btv.borrow().is_none() {
                return;
            }
            back_to_browse(
                &BackToBrowseCtx {
                    player: p_btv.clone(),
                    on_open: op_btv.clone(),
                    on_remove: rem_btv.clone(),
                    on_trash: trash_btv.clone(),
                    recent_backfill: rbf_btv.clone(),
                    last_path: last_btv.clone(),
                    sibling_seof: seof_btv.clone(),
                    win_aspect: wa_btv.clone(),
                    on_browse: browse_btv.clone(),
                    undo_shell: ush_btv.clone(),
                    undo_label: ula_btv.clone(),
                    undo_btn: undo_t_btv.clone(),
                    undo_timer: uti_btv.clone(),
                    undo_remove_stack: ur_btv.clone(),
                },
                &w_btv,
                &gl_btv,
                &recent_btv,
                &flow_btv,
                true,
            );
        }
    ));
    app.add_action(&close_video);
    *close_act_for_sync.borrow_mut() = Some(close_video.clone());
    let cv_s1 = close_video.clone();
    let p_s1 = player.clone();
    let r_s1 = recent_scrl.clone();
    recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
        sync_close_video_action(&cv_s1, &p_s1, &r_s1);
    });
    let cv_s2 = close_video.clone();
    let p_s2 = player.clone();
    let r_s2 = recent_scrl.clone();
    let _ = glib::idle_add_local_once(move || {
        sync_close_video_action(&cv_s2, &p_s2, &r_s2);
    });
    let close_video_rz = close_video.clone();

    let move_to_trash = gio::SimpleAction::new("move-to-trash", None);
    let p_mt = player.clone();
    let w_mt = win.clone();
    let recent_mt = recent_scrl.clone();
    let flow_mt = flow_recent.clone();
    let gl_mt = gl_area.clone();
    let op_mt = on_open.clone();
    let rem_mt = on_remove.clone();
    let trash_mt = on_trash.clone();
    let rbf_mt = recent_backfill.clone();
    let last_mt = last_path.clone();
    let seof_mt = sibling_seof.clone();
    let browse_mt = browse_chrome.clone();
    let wa_mt = win_aspect.clone();
    let ush_mt = undo_shell.clone();
    let ula_mt = undo_label.clone();
    let uti_mt = undo_timer.clone();
    let ur_mt = undo_remove_stack.clone();
    let undo_b_mt = undo_btn.clone();
    let do_mt = do_commit.clone();
    move_to_trash.connect_activate(glib::clone!(
        #[strong]
        p_mt,
        #[strong]
        w_mt,
        #[strong]
        recent_mt,
        #[strong]
        flow_mt,
        #[strong]
        gl_mt,
        #[strong]
        op_mt,
        #[strong]
        rem_mt,
        #[strong]
        trash_mt,
        #[strong]
        rbf_mt,
        #[strong]
        last_mt,
        #[strong]
        seof_mt,
        #[strong]
        browse_mt,
        #[strong]
        wa_mt,
        #[strong]
        ush_mt,
        #[strong]
        ula_mt,
        #[strong]
        uti_mt,
        #[strong]
        ur_mt,
        #[strong]
        undo_b_mt,
        #[strong]
        do_mt,
        move |_, _| {
            if recent_mt.is_visible() {
                return;
            }
            let path = {
                let g = p_mt.borrow();
                let Some(b) = g.as_ref() else {
                    return;
                };
                let Some(p) = local_file_from_mpv(&b.mpv) else {
                    return;
                };
                if !p.is_file() {
                    return;
                }
                p
            };
            let want = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let snap = capture_list_remove_undo(&path);
            let f = gio::File::for_path(&path);
            if let Err(e) = f.trash(gio::Cancellable::NONE) {
                eprintln!("[rhino] move to trash: {e}");
                return;
            }
            let in_trash = trash_xdg::find_trash_files_stored_path(&want);
            if in_trash.is_none() {
                eprintln!("[rhino] trash: could not locate trashed file for undo");
            }
            remove_continue_entry(&path);
            if let Some(t) = in_trash {
                ur_mt.borrow_mut().push(ContinueBarUndo::Trash { snap, in_trash: t });
            }
            back_to_browse(
                &BackToBrowseCtx {
                    player: p_mt.clone(),
                    on_open: op_mt.clone(),
                    on_remove: rem_mt.clone(),
                    on_trash: trash_mt.clone(),
                    recent_backfill: rbf_mt.clone(),
                    last_path: last_mt.clone(),
                    sibling_seof: seof_mt.clone(),
                    win_aspect: wa_mt.clone(),
                    on_browse: browse_mt.clone(),
                    undo_shell: ush_mt.clone(),
                    undo_label: ula_mt.clone(),
                    undo_btn: undo_b_mt.clone(),
                    undo_timer: uti_mt.clone(),
                    undo_remove_stack: ur_mt.clone(),
                },
                &w_mt,
                &gl_mt,
                &recent_mt,
                &flow_mt,
                false,
            );
            sync_undo_bar(&ula_mt, &undo_b_mt, &ush_mt, &ur_mt);
            if !ur_mt.borrow().is_empty() {
                rearm_undo_dismiss(&do_mt, uti_mt.as_ref());
            }
        }
    ));
    app.add_action(&move_to_trash);
    *trash_act_for_sync.borrow_mut() = Some(move_to_trash.clone());
    let mt_s1 = move_to_trash.clone();
    let p_mt1 = player.clone();
    let r_mt1 = recent_scrl.clone();
    recent_scrl.connect_notify_local(Some("visible"), move |_, _| {
        sync_trash_action(&mt_s1, &p_mt1, &r_mt1);
    });
    let mt_s2 = move_to_trash.clone();
    let p_mt2 = player.clone();
    let r_mt2 = recent_scrl.clone();
    let _ = glib::idle_add_local_once(move || {
        sync_trash_action(&mt_s2, &p_mt2, &r_mt2);
    });
    let move_trash_rz = move_to_trash.clone();

    let p_realize = player.clone();
    let sp_realize = sub_pref.clone();
    let vp_realize = Rc::clone(&video_pref);
    let app_realize = app.clone();
    let win_rz = win.clone();
    let gl_rz = gl_area.clone();
    let recent_rz = recent_scrl.clone();
    let bshow_rz = bar_show.clone();
    let bottom_rz = bottom.clone();
    let last_rz = last_path.clone();
    let on_vid_rz = on_video_chrome.clone();
    let ol_rz = Rc::clone(&on_file_loaded);
    let file_boot_rz = Rc::clone(&file_boot);
    let wa_st = Rc::clone(&win_aspect);
    let reapply_rz = reapply_60.clone();
    let pending_rz = pending_recent_backfill.clone();
    gl_area.connect_realize(move |area| {
        area.make_current();
        let init = {
            let mut v = vp_realize.borrow_mut();
            MpvBundle::new(area, &mut v)
        };
        match init {
            Ok((b, auto_off)) => {
                if auto_off {
                    sync_smooth_60_to_off(&app_realize);
                }
                let (av, am) = db::load_audio();
                let _ = b.mpv.set_property("volume", av);
                let _ = b.mpv.set_property("mute", am);
                {
                    let s = sp_realize.borrow();
                    sub_prefs::apply_mpv(&b.mpv, &s);
                }
                *p_realize.borrow_mut() = Some(b);
                let preload_auto_off = preload_first_continue(&p_realize, &vp_realize, &recent_rz);
                if preload_auto_off == Some(true) {
                    sync_smooth_60_to_off(&app_realize);
                }
                if preload_auto_off.is_some() {
                    let p_pause = p_realize.clone();
                    let r_pause = recent_rz.clone();
                    let _ = glib::timeout_add_local(Duration::from_millis(100), move || {
                        if r_pause.is_visible() {
                            if let Some(b) = p_pause.borrow().as_ref() {
                                let _ = b.mpv.set_property("pause", true);
                            }
                        }
                        glib::ControlFlow::Break
                    });
                    let p_60 = p_realize.clone();
                    let r_60 = reapply_rz.clone();
                    let rec_60 = recent_rz.clone();
                    let app_60 = app_realize.clone();
                    let _ = glib::idle_add_local_once(move || {
                        if !rec_60.is_visible() {
                            return;
                        }
                        if let Some(b) = p_60.borrow().as_ref() {
                            let off = {
                                let mut g = r_60.vp.borrow_mut();
                                video_pref::reapply_60_if_still_missing(&b.mpv, &mut g)
                            };
                            if off {
                                sync_smooth_60_to_off(&app_60);
                            }
                        }
                    });
                }
                drain_recent_backfill(&pending_rz);
                sync_close_video_action(&close_video_rz, &p_realize, &recent_rz);
                sync_trash_action(&move_trash_rz, &p_realize, &recent_rz);
                if let Some(pl) = p_realize.borrow().as_ref() {
                    let show = if recent_rz.is_visible() { true } else { bshow_rz.get() };
                    sub_prefs::apply_sub_pos_for_toolbar(
                        &pl.mpv,
                        show,
                        bottom_rz.height(),
                        area.height(),
                    );
                }
                if let Some(bundle) = p_realize.borrow_mut().as_mut() {
                    let _ = bundle.mpv.disable_deprecated_events();
                }
                if let Some(p) = file_boot_rz.replace(None) {
                    if let Err(e) = try_load(
                        &p,
                        &p_realize,
                        &win_rz,
                        &gl_rz,
                        &recent_rz,
                        &LoadOpts {
                            record: true,
                            play_on_start: false,
                            last_path: last_rz.clone(),
                            on_start: Some(Rc::clone(&on_vid_rz)),
                            win_aspect: wa_st.clone(),
                            on_loaded: Some(Rc::clone(&ol_rz)),
                            reapply_60: Some(reapply_rz.clone()),
                        },
                    ) {
                        eprintln!("[rhino] try_load (startup): {e}");
                    }
                }
            }
            Err(e) => eprintln!("[rhino] OpenGL / mpv: {e}"),
        }
    });

    let p_draw = player.clone();
    gl_area.connect_render(move |area, _ctx| {
        area.make_current();
        if let Some(b) = p_draw.borrow().as_ref() {
            b.draw(area);
        }
        glib::Propagation::Stop
    });

    let seek_sync = Rc::new(Cell::new(false));
    let p_seek = player.clone();
    seek.connect_value_changed(glib::clone!(
        #[strong]
        p_seek,
        #[strong]
        seek_sync,
        move |r| {
            if seek_sync.get() {
                return;
            }
            if let Some(b) = p_seek.borrow().as_ref() {
                let s = format!("{:.4}", r.value());
                if b.mpv.command("seek", &[s.as_str(), "absolute+keyframes"]).is_err() {
                    let _ = b.mpv.set_property("time-pos", r.value());
                }
            }
        }
    ));

    let vol_sync = Rc::new(Cell::new(false));
    let p_vctl = player.clone();
    let vi = vol_menu.clone();
    let vm = vol_mute_btn.clone();
    let vsx = vol_sync.clone();
    vol_adj.connect_value_changed(glib::clone!(
        #[strong]
        p_vctl,
        #[strong]
        vi,
        #[strong]
        vm,
        #[strong]
        vsx,
        move |a| {
            if vsx.get() {
                return;
            }
            if let Some(b) = p_vctl.borrow().as_ref() {
                let v = a.value();
                let _ = b.mpv.set_property("volume", v);
                if v > 0.5 {
                    let _ = b.mpv.set_property("mute", false);
                }
                let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
                let cur = b.mpv.get_property::<f64>("volume").unwrap_or(v);
                vi.set_icon_name(vol_icon(m, cur));
                vsx.set(true);
                if vm.is_active() != m {
                    vm.set_active(m);
                }
                vm.set_icon_name(vol_mute_pop_icon(m));
                vm.set_tooltip_text(Some(if m { "Unmute" } else { "Mute" }));
                vsx.set(false);
            }
        }
    ));
    let p_mute = player.clone();
    let vi2 = vol_menu.clone();
    let vsx2 = vol_sync.clone();
    vol_mute_btn.connect_toggled(glib::clone!(
        #[strong]
        p_mute,
        #[strong]
        vi2,
        #[strong]
        vsx2,
        move |ch| {
            if vsx2.get() {
                return;
            }
            if let Some(b) = p_mute.borrow().as_ref() {
                let m = ch.is_active();
                let _ = b.mpv.set_property("mute", m);
                let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
                vi2.set_icon_name(vol_icon(m, vol));
                ch.set_icon_name(vol_mute_pop_icon(m));
                ch.set_tooltip_text(Some(if m { "Unmute" } else { "Mute" }));
            }
        }
    ));

    {
        let p = player.clone();
        let r = recent_scrl.clone();
        let vmi = vol_menu.clone();
        let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        sc.set_propagation_phase(gtk::PropagationPhase::Target);
        sc.connect_scroll(glib::clone!(
            #[strong]
            p,
            #[strong]
            r,
            #[strong]
            vmi,
            move |_, _dx, dy| {
                if r.is_visible() {
                    return glib::Propagation::Proceed;
                }
                let g = p.borrow();
                let Some(b) = g.as_ref() else {
                    return glib::Propagation::Proceed;
                };
                let step = if dy.abs() < 0.5 {
                    -dy * 4.0
                } else {
                    -dy * 5.0
                };
                nudge_mpv_volume(&b.mpv, step);
                let vol = b.mpv.get_property::<f64>("volume").unwrap_or(0.0);
                let m = b.mpv.get_property::<bool>("mute").unwrap_or(false);
                vmi.set_icon_name(vol_icon(m, vol));
                glib::Propagation::Stop
            }
        ));
        gl_area.add_controller(sc);
    }

    {
        let deb = aspect_resize_end_deb.clone();
        let wired = aspect_resize_wired.clone();
        let w = win.clone();
        let r = recent_scrl.clone();
        let wa = win_aspect.clone();
        w.connect_map(glib::clone!(
            #[strong]
            w,
            #[strong]
            r,
            #[strong]
            wa,
            #[strong]
            deb,
            #[strong]
            wired,
            move |_| {
                if wired.get() {
                    return;
                }
                let on_resize: Rc<dyn Fn()> = Rc::new(glib::clone!(
                    #[strong]
                    deb,
                    #[strong]
                    w,
                    #[strong]
                    r,
                    #[strong]
                    wa,
                    move || schedule_window_aspect_on_resize_end(Rc::clone(&deb), &w, &r, &wa)
                ));
                let Some(n) = w.native() else {
                    return;
                };
                let Some(surf) = n.surface() else {
                    return;
                };
                surf.connect_width_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                surf.connect_height_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                let gw: &gtk::Window = w.upcast_ref();
                gw.connect_default_width_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                gw.connect_default_height_notify(glib::clone!(
                    #[strong]
                    on_resize,
                    move |_| on_resize()
                ));
                wired.set(true);
                if aspect_debug() {
                    eprintln!(
                        "[rhino] aspect: resize-end hooks (GdkSurface + GtkWindow default size)"
                    );
                }
            }
        ));
    }

    let p_poll = player.clone();
    let win_poll = win.clone();
    let gl_poll = gl_area.clone();
    let rec_poll = recent_scrl.clone();
    let last_poll = last_path.clone();
    let seof_poll = sibling_seof.clone();
    let wa_poll = win_aspect.clone();
    let s_flag = seek_sync.clone();
    let tw_l = time_left.downgrade();
    let tw_r = time_right.downgrade();
    let ppw = play_pause.downgrade();
    // `set_tooltip_text` on a sensitive parent: inactive buttons do not get pointer events, so
    // the default `query-tooltip` path never runs on the child (see GtkWidget `set_can_target`).
    let wpw_prev = wrap_prev.downgrade();
    let wpw_next = wrap_next.downgrade();
    let bpw_prev = btn_prev.downgrade();
    let bpw_next = btn_next.downgrade();
    let spdm = speed_mbtn.downgrade();
    let sw = seek.clone();
    let adj = seek_adj.clone();
    let vi_poll = vol_menu.clone();
    let vadj_p = vol_adj.clone();
    let vm_p = vol_mute_btn.clone();
    let vsy = vol_sync.clone();
    let on_poll = on_video_chrome.clone();
    glib::timeout_add_local(
        Duration::from_millis(200),
        glib::clone!(
            #[strong]
            p_poll,
            #[strong]
            win_poll,
            #[strong]
            gl_poll,
            #[strong]
            rec_poll,
            #[strong]
            last_poll,
            #[strong]
            seof_poll,
            #[strong]
            on_poll,
            #[strong]
            wa_poll,
            #[strong]
            on_file_loaded,
            #[strong]
            reapply_60,
            #[strong]
            seek_state,
            #[strong]
            spdm,
            move || {
                maybe_advance_sibling_on_eof(
                    &p_poll,
                    &win_poll,
                    &gl_poll,
                    &rec_poll,
                    &last_poll,
                    seof_poll.as_ref(),
                    &on_poll,
                    Rc::clone(&wa_poll),
                    Some(Rc::clone(&on_file_loaded)),
                    &reapply_60,
                );
                let Some(tl) = tw_l.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(tr) = tw_r.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                // Drain seek-preview channel even when no media (pending thread after close).
                seek_state.on_tick();
                let g = p_poll.borrow();
                let Some(pl) = g.as_ref() else {
                    seof_poll.clear_nav_sensitivity();
                    if let Some(pp) = ppw.upgrade() {
                        pp.set_sensitive(false);
                        pp.set_icon_name("media-playback-start-symbolic");
                        pp.set_tooltip_text(Some("No media"));
                    }
                    if let Some(w) = wpw_prev.upgrade() {
                        w.set_tooltip_text(Some("No media"));
                    }
                    if let Some(p) = bpw_prev.upgrade() {
                        p.set_sensitive(false);
                        p.set_can_target(false);
                    }
                    if let Some(w) = wpw_next.upgrade() {
                        w.set_tooltip_text(Some("No media"));
                    }
                    if let Some(n) = bpw_next.upgrade() {
                        n.set_sensitive(false);
                        n.set_can_target(false);
                    }
                    if let Some(sb) = spdm.upgrade() {
                        sb.set_sensitive(false);
                    }
                    return glib::ControlFlow::Continue;
                };
                sync_window_aspect_from_mpv(&pl.mpv, wa_poll.as_ref());
                let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
                tl.set_label(&format_time(pos));
                tr.set_label(&format_time(dur));
                if let Some(pp) = ppw.upgrade() {
                    if dur > 0.0 {
                        pp.set_sensitive(true);
                        let paused = pl.mpv.get_property::<bool>("pause").unwrap_or(false);
                        if paused {
                            pp.set_icon_name("media-playback-start-symbolic");
                            pp.set_tooltip_text(Some("Play (Space)"));
                        } else {
                            pp.set_icon_name("media-playback-pause-symbolic");
                            pp.set_tooltip_text(Some("Pause (Space)"));
                        }
                    } else {
                        pp.set_sensitive(false);
                        pp.set_icon_name("media-playback-start-symbolic");
                        pp.set_tooltip_text(Some("No media"));
                    }
                }
                let cur = if dur > 0.0 {
                    local_file_from_mpv(&pl.mpv).or_else(|| last_poll.borrow().clone())
                } else {
                    None
                };
                let (can_prev, can_next) = if dur > 0.0 {
                    if let Some(c) = cur.as_ref().filter(|p| p.is_file()) {
                        seof_poll.nav_sensitivity(c)
                    } else {
                        seof_poll.clear_nav_sensitivity();
                        (false, false)
                    }
                } else {
                    seof_poll.clear_nav_sensitivity();
                    (false, false)
                };
                if let Some(p) = bpw_prev.upgrade() {
                    p.set_sensitive(can_prev);
                    p.set_can_target(can_prev);
                }
                if let Some(w) = wpw_prev.upgrade() {
                    let tip = sibling_bar_tooltip(true, can_prev, cur.as_deref());
                    w.set_tooltip_text(Some(tip.as_str()));
                }
                if let Some(n) = bpw_next.upgrade() {
                    n.set_sensitive(can_next);
                    n.set_can_target(can_next);
                }
                if let Some(w) = wpw_next.upgrade() {
                    let tip = sibling_bar_tooltip(false, can_next, cur.as_deref());
                    w.set_tooltip_text(Some(tip.as_str()));
                }
                if dur > 0.0 {
                    sw.set_sensitive(true);
                    if let Some(sb) = spdm.upgrade() {
                        sb.set_sensitive(true);
                    }
                    adj.set_lower(0.0);
                    adj.set_upper(dur);
                    s_flag.set(true);
                    adj.set_value(pos.clamp(0.0, dur));
                    s_flag.set(false);
                } else {
                    sw.set_sensitive(false);
                    if let Some(sb) = spdm.upgrade() {
                        sb.set_sensitive(false);
                    }
                }
                let vol = pl.mpv.get_property::<f64>("volume").unwrap_or(0.0);
                let muted = pl.mpv.get_property::<bool>("mute").unwrap_or(false);
                vi_poll.set_icon_name(vol_icon(muted, vol));
                if !vi_poll.is_active() {
                    let vmax = pl.mpv.get_property::<f64>("volume-max").unwrap_or(100.0);
                    if vmax.is_finite() && vmax > 0.0 {
                        vadj_p.set_upper(vmax);
                    }
                    vsy.set(true);
                    vadj_p.set_value(vol.clamp(0.0, vadj_p.upper()));
                    if vm_p.is_active() != muted {
                        vm_p.set_active(muted);
                    }
                    vm_p.set_icon_name(vol_mute_pop_icon(muted));
                    vm_p.set_tooltip_text(Some(if muted { "Unmute" } else { "Mute" }));
                    vsy.set(false);
                }
                glib::ControlFlow::Continue
            }
        ),
    );

    // Open
    let open = gio::SimpleAction::new("open", None);
    let p_open = player.clone();
    let gl_w = gl_area.clone();
    let recent_choose = recent_scrl.clone();
    let last_filepicker = last_path.clone();
    let ovc_open = on_video_chrome.clone();
    let wa_dlg = Rc::clone(&win_aspect);
    open.connect_activate(glib::clone!(
        #[weak]
        app,
        #[strong]
        ovc_open,
        #[strong]
        wa_dlg,
        #[strong]
        on_file_loaded,
        #[strong]
        reapply_60,
        move |_, _| {
            let Some(w) = app.active_window() else {
                return;
            };
            let vf = video_file_filter();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&vf);
            let dialog = gtk::FileDialog::builder()
                .title("Open video")
                .modal(true)
                .filters(&filters)
                .default_filter(&vf)
                .build();
            let p_c = p_open.clone();
            let w_f = w.clone();
            let gl_w = gl_w.clone();
            let recent_choose = recent_choose.clone();
            let last_fp = last_filepicker.clone();
            let ovc2 = ovc_open.clone();
            let wa2 = Rc::clone(&wa_dlg);
            let oload = Rc::clone(&on_file_loaded);
            let re_o = reapply_60.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else {
                    return;
                };
                let Some(path) = file.path() else {
                    eprintln!("[rhino] open: non-path URIs not implemented yet");
                    return;
                };
                let Some(aw) = w_f.downcast_ref::<adw::ApplicationWindow>() else {
                    return;
                };
                if let Err(e) = try_load(
                    &path,
                    &p_c,
                    aw,
                    &gl_w,
                    &recent_choose,
                    &LoadOpts {
                        record: true,
                        play_on_start: true,
                        last_path: last_fp.clone(),
                        on_start: Some(ovc2),
                        win_aspect: wa2.clone(),
                        on_loaded: Some(oload),
                        reapply_60: Some(re_o.clone()),
                    },
                ) {
                    eprintln!("[rhino] open: try_load: {e}");
                }
            });
        }
    ));
    app.add_action(&open);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let parent = app.active_window();
            let mut b = gtk::AboutDialog::builder()
                .program_name("Rhino Player")
                .version(env!("CARGO_PKG_VERSION"))
                .copyright("Copyright (C) 2026 Peter Adrianov")
                .logo_icon_name(APP_ID)
                .comments("mpv with GTK 4 and libadwaita.")
                .license(LICENSE_NOTICE)
                .license_type(gtk::License::Custom)
                .website("https://github.com/adrianov/rhino-player")
                .modal(true);
            if let Some(ref w) = parent {
                b = b.transient_for(w);
            }
            b.build().present();
        }
    ));
    app.add_action(&about);

    let app_q = app.clone();
    let quit = gio::SimpleAction::new("quit", None);
    let p_quit = player.clone();
    let win_q = win.clone();
    let sp_quit = sub_pref.clone();
    let idle_q = Rc::clone(&idle_inhib);
    quit.connect_activate(glib::clone!(
        #[strong]
        app_q,
        #[strong]
        p_quit,
        #[strong]
        win_q,
        #[strong]
        sp_quit,
        #[strong]
        idle_q,
        move |_, _| {
            schedule_quit_persist(&app_q, &win_q, &p_quit, &sp_quit, &idle_q);
        }
    ));
    app.add_action(&quit);

    register_video_app_actions(
        app,
        &win,
        &gl_area,
        player,
        Rc::clone(&video_pref),
        &pref_menu,
        Rc::clone(&seek_bar_on),
    );

    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.close-video", &["<Primary>w"]);
    app.set_accels_for_action("app.move-to-trash", &["Delete", "KP_Delete"]);
    app.set_accels_for_action("app.about", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Primary>q", "q"]);

    {
        let p = player.clone();
        let w = win.clone();
        let sp_close = sub_pref.clone();
        let iclose = Rc::clone(&idle_inhib);
        win.connect_close_request(glib::clone!(
            #[strong]
            app_q,
            #[strong]
            p,
            #[strong]
            w,
            #[strong]
            sp_close,
            #[strong]
            iclose,
            move |_win| {
                schedule_quit_persist(&app_q, &w, &p, &sp_close, &iclose);
                glib::Propagation::Stop
            }
        ));
    }

    apply_chrome(
        &root,
        &gl_area,
        &bar_show,
        &recent_scrl,
        &bottom,
        player,
    );
    {
        let pz = player.clone();
        let bz = bar_show.clone();
        let rz = recent_scrl.clone();
        let botz = bottom.clone();
        let glz = gl_area.clone();
        let on_sz = Rc::new(move || {
            if let Some(b) = pz.borrow().as_ref() {
                let show = if rz.is_visible() { true } else { bz.get() };
                sub_prefs::apply_sub_pos_for_toolbar(&b.mpv, show, botz.height(), glz.height());
            }
        });
        let a = Rc::clone(&on_sz);
        let b = on_sz;
        gl_area.connect_notify_local(Some("height"), move |_, _| a());
        bottom.connect_notify_local(Some("height"), move |_, _| b());
    }

    {
        let idle_t = Rc::clone(&idle_inhib);
        let p_t = Rc::clone(player);
        let r_t = recent_scrl.clone();
        let a_t = app.clone();
        let w_t = win.clone();
        glib::source::timeout_add_local(
            Duration::from_millis(500),
            glib::clone!(
                #[strong] a_t,
                #[strong] w_t,
                #[strong] p_t,
                #[strong] r_t,
                #[strong] idle_t,
                move || {
                    let should = idle_inhibit::should_inhibit(&p_t, r_t.is_visible());
                    let gtk_a: &gtk::Application = a_t.upcast_ref();
                    idle_inhibit::sync(gtk_a, Some(&w_t), should, &idle_t);
                    glib::ControlFlow::Continue
                }
            ),
        );
    }

    win.present();
}
