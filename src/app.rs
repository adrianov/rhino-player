use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::db;
use crate::format_time;
use crate::history;
use libmpv2::Mpv;
use crate::media_probe::{card_data_list, record_playback_for_current, CardData};
use crate::mpv_embed::MpvBundle;
use crate::recent_view;
use crate::theme;

const APP_ID: &str = "ch.rhino.RhinoPlayer";
const IDLE_3S: Duration = Duration::from_secs(3);
/// After chrome hides, GTK often emits spurious pointer motion/enter; ignore for this long.
const LAYOUT_SQUELCH: Duration = Duration::from_millis(450);
/// Ignore repeated motion with the same coordinates (reflows can re-emit the same (x, y)).
const COORD_EPS: f64 = 1.0;

type RcPathFn = Rc<dyn Fn(&Path)>;
type StaleSlot = Rc<RefCell<Option<RcPathFn>>>;

fn same_xy(a: f64, b: f64) -> bool {
    (a - b).abs() < COORD_EPS
}

fn show_pointer(gl: &gtk::GLArea) {
    gl.remove_css_class("rp-cursor-hidden");
    gl.set_cursor_from_name(None);
}

fn toggle_fullscreen(win: &adw::ApplicationWindow) {
    if win.is_fullscreen() {
        win.unfullscreen();
    } else if win.is_maximized() {
        win.unmaximize();
        win.fullscreen();
    } else {
        win.fullscreen();
    }
}

/// Chrome layout: when not fullscreen, always show. When fullscreen, only when
/// `fs_overlay` is true (moved mouse recently) unless `apply_chrome` is called from
/// fullscreened_notify with overlay cleared.
///
/// In fullscreen, `AdwToolbarView` content is extended to the top and bottom *edges* so
/// the `GLArea` can fill the whole area; the header and bottom bar draw on top of the
/// video (overlay), instead of compressing the video.
fn apply_chrome(
    win: &adw::ApplicationWindow,
    root: &adw::ToolbarView,
    status: &gtk::Label,
    gl: &gtk::GLArea,
    fs_overlay: &Cell<bool>,
) {
    let fs = win.is_fullscreen();
    let show = if fs { fs_overlay.get() } else { true };
    if fs {
        root.set_extend_content_to_top_edge(true);
        root.set_extend_content_to_bottom_edge(true);
    } else {
        root.set_extend_content_to_top_edge(false);
        root.set_extend_content_to_bottom_edge(false);
    }
    root.set_reveal_top_bars(show);
    root.set_reveal_bottom_bars(show);
    status.set_visible(show);
    gl.queue_render();
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

pub fn run() -> i32 {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }

    if let Err(e) = adw::init() {
        eprintln!("libadwaita: {e}");
        return 1;
    }

    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_app| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        db::init();
        theme::apply();
    });

    let player: Rc<RefCell<Option<MpvBundle>>> = Rc::new(RefCell::new(None));

    {
        let p = player.clone();
        app.connect_activate(move |a: &adw::Application| {
            if a.windows().is_empty() {
                let startup = std::env::args().nth(1).map(PathBuf::from);
                build_window(a, &p, startup);
            }
        });
    }
    app.run().into()
}

/// Load a file, hide the recent grid overlay, show video; [record] appends to recent history.
fn try_load(
    path: &Path,
    player: &RefCell<Option<MpvBundle>>,
    st: &gtk::Label,
    gl: &gtk::GLArea,
    recent_layer: &impl IsA<gtk::Widget>,
    record: bool,
) -> Result<(), String> {
    eprintln!(
        "[rhino] try_load: path={} exists={} record={} player_ready={}",
        path.display(),
        path.exists(),
        record,
        player.borrow().is_some()
    );
    let mut g = player.borrow_mut();
    let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
    if let Err(e) = b.load_file_path(path) {
        eprintln!("[rhino] try_load: loadfile failed: {e}");
        return Err(e);
    }
    eprintln!("[rhino] try_load: loadfile ok");
    if record {
        history::record(path);
    }
    st.set_label(&format!("Loaded: {}", path.display()));
    recent_layer.set_visible(false);
    gl.queue_render();
    Ok(())
}

fn save_mpv_audio(mpv: &Mpv) {
    let vol = mpv.get_property::<f64>("volume").unwrap_or(100.0);
    let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
    db::save_audio(vol, muted);
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

fn nudge_mpv_volume(mpv: &Mpv, delta: f64) {
    let max = mpv.get_property::<f64>("volume-max").unwrap_or(100.0).max(1.0);
    let cur = mpv.get_property::<f64>("volume").unwrap_or(0.0);
    let nv = (cur + delta).clamp(0.0, max);
    let _ = mpv.set_property("volume", nv);
    if nv > 0.5 {
        let _ = mpv.set_property("mute", false);
    }
}

/// Show the sheet immediately; mpv/DB/grid/stop on LOW-priority idles (after a frame paints).
fn back_to_browse(
    player: Rc<RefCell<Option<MpvBundle>>>,
    st: &gtk::Label,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    row: &gtk::Box,
    on_open: RcPathFn,
    on_stale: RcPathFn,
) {
    let paths: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if paths.is_empty() {
        recent.set_visible(false);
    } else {
        recent.set_visible(true);
    }
    st.set_label("Open a file (Ctrl+O).");
    gl.queue_render();

    if paths.is_empty() {
        let p2 = player.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(b) = p2.borrow().as_ref() {
                b.write_resume_snapshot();
                record_playback_for_current(&b.mpv);
                b.persist_on_quit();
                b.stop_playback();
            }
            glib::ControlFlow::Break
        });
        return;
    }

    // FnOnce chain: `idle_add_local_full` requires FnMut, so the second/third steps are
    // scheduled from a one-shot idle (paint can run first at DEFAULT_IDLE priority).
    let p_write = player.clone();
    let row2 = row.clone();
    let op2 = on_open;
    let osl2 = on_stale;
    let paths2 = paths;
    let _ = glib::source::idle_add_local_once(move || {
        if let Some(b) = p_write.borrow().as_ref() {
            b.write_resume_snapshot();
            record_playback_for_current(&b.mpv);
        }
        let p3 = p_write.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            recent_view::fill_row(&row2, v, op2.clone(), osl2.clone());
            let p4 = p3.clone();
            let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
                if let Some(b) = p4.borrow().as_ref() {
                    b.persist_on_quit();
                    b.stop_playback();
                }
                glib::ControlFlow::Break
            });
            glib::ControlFlow::Break
        });
    });
}

/// Hides the window, then (after GTK can draw the hide) saves watch_later/DB, stops, and quits.
fn schedule_quit_persist(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    player: &Rc<RefCell<Option<MpvBundle>>>,
) {
    win.set_visible(false);
    let p = player.clone();
    let a = app.clone();
    let _ = glib::idle_add_local(move || {
        if let Some(b) = p.borrow().as_ref() {
            save_mpv_audio(&b.mpv);
            b.commit_quit();
        }
        a.quit();
        glib::ControlFlow::Break
    });
}

fn build_window(
    app: &adw::Application,
    player: &Rc<RefCell<Option<MpvBundle>>>,
    startup: Option<PathBuf>,
) {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Rhino Player")
        .default_width(960)
        .default_height(540)
        .css_classes(["rp-win"])
        .build();

    let fs_overlay = Rc::new(Cell::new(false));
    let nav_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let cur_t = Rc::new(RefCell::new(None::<glib::SourceId>));
    let ptr_in_gl = Rc::new(Cell::new(false));
    let motion_squelch = Rc::new(Cell::new(None::<Instant>));
    let last_cap_xy = Rc::new(Cell::new(None::<(f64, f64)>));
    let last_gl_xy = Rc::new(Cell::new(None::<(f64, f64)>));

    let root = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    let menu = gio::Menu::new();
    menu.append(Some("Open…"), Some("app.open"));
    menu.append(Some("About Rhino Player"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let vol_adj = gtk::Adjustment::new(100.0, 0.0, 100.0, 1.0, 5.0, 0.0);
    let vol_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&vol_adj));
    vol_scale.set_draw_value(true);
    vol_scale.set_hexpand(true);
    vol_scale.set_size_request(220, -1);
    vol_scale.add_css_class("rp-vol");
    let vol_mute = gtk::CheckButton::with_label("Mute");
    let vol_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vol_box.set_margin_start(12);
    vol_box.set_margin_end(12);
    vol_box.set_margin_top(12);
    vol_box.set_margin_bottom(12);
    vol_box.append(&vol_scale);
    vol_box.append(&vol_mute);
    let vol_pop = gtk::Popover::new();
    vol_pop.set_child(Some(&vol_box));
    let vol_menu = gtk::MenuButton::new();
    vol_menu.set_icon_name("audio-volume-high-symbolic");
    vol_menu.set_tooltip_text(Some("Volume"));
    vol_menu.set_popover(Some(&vol_pop));

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some("Main menu"));
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);
    header.pack_end(&vol_menu);

    let status = gtk::Label::new(Some("Initializing video…"));
    status.add_css_class("rp-status");
    status.set_wrap(true);
    status.set_wrap_mode(gtk::pango::WrapMode::Word);
    status.set_xalign(0.0);
    status.set_vexpand(false);
    status.set_halign(gtk::Align::Fill);
    status.set_valign(gtk::Align::Start);
    status.set_can_target(false);

    let gl_area = gtk::GLArea::new();
    gl_area.add_css_class("rp-gl");
    gl_area.set_hexpand(true);
    gl_area.set_vexpand(true);
    gl_area.set_auto_render(false);
    gl_area.set_has_stencil_buffer(false);
    gl_area.set_has_depth_buffer(false);

    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    dbl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let win_fs = win.clone();
        dbl.connect_pressed(move |_, n_press, _, _| {
            if n_press == 2 {
                toggle_fullscreen(&win_fs);
            }
        });
    }
    gl_area.add_controller(dbl);

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
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);

    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(&gl_area));
    ovl.add_overlay(&status);
    ovl.set_measure_overlay(&status, false);

    let (recent_scrl, flow_recent) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);

    let want_recent = startup.is_none() && !history::load().is_empty();
    recent_scrl.set_visible(want_recent);

    let p_openr = player.clone();
    let st_op = status.clone();
    let gl_op = gl_area.clone();
    let recent_on_top = recent_scrl.clone();
    let on_open: RcPathFn = Rc::new(move |path: &Path| {
        eprintln!("[rhino] on_open from recent/menu: {}", path.display());
        if let Err(e) = try_load(path, p_openr.as_ref(), &st_op, &gl_op, &recent_on_top, true) {
            eprintln!("[rhino] on_open: try_load error: {e}");
            st_op.set_label(&e);
        }
    });

    let h_sl: StaleSlot = Rc::new(RefCell::new(None));
    let h2 = h_sl.clone();
    let fr_sl = flow_recent.clone();
    let recent_stale = recent_scrl.clone();
    let op_s = on_open.clone();
    h_sl.borrow_mut().replace(Rc::new(move |path: &Path| {
        history::remove(path);
        let r: Vec<PathBuf> = history::load().into_iter().take(5).collect();
        if r.is_empty() {
            recent_stale.set_visible(false);
            return;
        }
        let v: Vec<CardData> = card_data_list(&r);
        if let Some(s) = h2.borrow().as_ref() {
            recent_view::fill_row(&fr_sl, v, op_s.clone(), s.clone());
        }
    }));
    let on_stale = h_sl.borrow().as_ref().unwrap().clone();

    if want_recent {
        let paths5: Vec<PathBuf> = history::load().into_iter().take(5).collect();
        recent_view::fill_idle(&flow_recent, paths5, on_open.clone(), on_stale.clone());
    }

    root.add_top_bar(&header);
    root.set_content(Some(&ovl));
    root.add_bottom_bar(&bottom);

    win.set_content(Some(&root));

    {
        let root_fs = root.clone();
        let st_fs = status.clone();
        let gl_fs = gl_area.clone();
        let fov = fs_overlay.clone();
        let nav = nav_t.clone();
        let sq = motion_squelch.clone();
        let lcap = last_cap_xy.clone();
        let lgl = last_gl_xy.clone();
        win.connect_fullscreened_notify(move |w| {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            sq.set(None);
            lcap.set(None);
            lgl.set(None);
            fov.set(false);
            apply_chrome(w, &root_fs, &st_fs, &gl_fs, &fov);
        });
    }

    {
        let win_c = win.clone();
        let root_c = root.clone();
        let st_c = status.clone();
        let gl_c = gl_area.clone();
        let fov = fs_overlay.clone();
        let nav = nav_t.clone();
        let sq = motion_squelch.clone();
        let lcap = last_cap_xy.clone();
        let cap = gtk::EventControllerMotion::new();
        cap.set_propagation_phase(gtk::PropagationPhase::Capture);
        cap.connect_motion(glib::clone!(
            #[strong]
            win_c,
            #[strong]
            root_c,
            #[strong]
            st_c,
            #[strong]
            gl_c,
            #[strong]
            fov,
            #[strong]
            nav,
            #[strong]
            sq,
            #[strong]
            lcap,
            move |_, x, y| {
                if !win_c.is_fullscreen() {
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

                fov.set(true);
                apply_chrome(&win_c, &root_c, &st_c, &gl_c, &fov);
                replace_timeout(nav.clone(), {
                    let win2 = win_c.clone();
                    let root2 = root_c.clone();
                    let st2 = st_c.clone();
                    let gl2 = gl_c.clone();
                    let f2 = fov.clone();
                    let sq2 = sq.clone();
                    move || {
                        if !win2.is_fullscreen() {
                            return;
                        }
                        f2.set(false);
                        apply_chrome(&win2, &root2, &st2, &gl2, &f2);
                        sq2.set(Some(Instant::now() + LAYOUT_SQUELCH));
                    }
                });
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
        let st_esc = status.clone();
        let gl_esc = gl_area.clone();
        let op_esc = on_open.clone();
        let stl_esc = on_stale.clone();
        let k = gtk::EventControllerKey::new();
        k.connect_key_pressed(move |_, key, _code, _m| {
            if key == gtk::gdk::Key::Escape {
                if win_key.is_fullscreen() {
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
                    p.clone(),
                    &st_esc,
                    &gl_esc,
                    &recent_esc,
                    &flow_esc,
                    op_esc.clone(),
                    stl_esc.clone(),
                );
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
                toggle_fullscreen(&win_key);
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

    let p_realize = player.clone();
    let st_realize = status.clone();
    let gl_rz = gl_area.clone();
    let recent_rz = recent_scrl.clone();
    let st_path = startup;
    gl_area.connect_realize(move |area| {
        area.make_current();
        match MpvBundle::new(area) {
            Ok(b) => {
                let (av, am) = db::load_audio();
                let _ = b.mpv.set_property("volume", av);
                let _ = b.mpv.set_property("mute", am);
                *p_realize.borrow_mut() = Some(b);
                if let Some(ref p) = st_path {
                    if let Err(e) =
                        try_load(p, p_realize.as_ref(), &st_realize, &gl_rz, &recent_rz, true)
                    {
                        st_realize.set_label(&e);
                    }
                } else {
                    st_realize.set_label("Open a file (Ctrl+O).");
                }
            }
            Err(e) => st_realize.set_label(&format!("OpenGL / mpv: {e}")),
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
                let _ = b.mpv.set_property("time-pos", r.value());
            }
        }
    ));

    let vol_sync = Rc::new(Cell::new(false));
    let p_vctl = player.clone();
    let vi = vol_menu.clone();
    let vm = vol_mute.clone();
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
                vsx.set(false);
            }
        }
    ));
    let p_mute = player.clone();
    let vi2 = vol_menu.clone();
    let vsx2 = vol_sync.clone();
    vol_mute.connect_toggled(glib::clone!(
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

    let p_poll = player.clone();
    let s_flag = seek_sync.clone();
    let tw_l = time_left.downgrade();
    let tw_r = time_right.downgrade();
    let sw = seek.clone();
    let adj = seek_adj.clone();
    let vi_poll = vol_menu.clone();
    let vadj_p = vol_adj.clone();
    let vm_p = vol_mute.clone();
    let vsy = vol_sync.clone();
    glib::timeout_add_local(
        Duration::from_millis(200),
        glib::clone!(
            #[strong]
            p_poll,
            move || {
                let Some(tl) = tw_l.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(tr) = tw_r.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let g = p_poll.borrow();
                let Some(pl) = g.as_ref() else {
                    return glib::ControlFlow::Continue;
                };
                let pos = pl.mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                let dur = pl.mpv.get_property::<f64>("duration").unwrap_or(0.0);
                tl.set_label(&format_time(pos));
                tr.set_label(&format_time(dur));
                if dur > 0.0 {
                    sw.set_sensitive(true);
                    adj.set_lower(0.0);
                    adj.set_upper(dur);
                    s_flag.set(true);
                    adj.set_value(pos.clamp(0.0, dur));
                    s_flag.set(false);
                } else {
                    sw.set_sensitive(false);
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
                    vsy.set(false);
                }
                glib::ControlFlow::Continue
            }
        ),
    );

    // Open
    let open = gio::SimpleAction::new("open", None);
    let p_open = player.clone();
    let st = status.clone();
    let gl_w = gl_area.clone();
    let recent_choose = recent_scrl.clone();
    open.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let Some(w) = app.active_window() else {
                return;
            };
            let dialog = gtk::FileDialog::builder()
                .title("Open media")
                .modal(true)
                .build();
            let p_c = p_open.clone();
            let st = st.clone();
            let gl_w = gl_w.clone();
            let recent_choose = recent_choose.clone();
            dialog.open(Some(&w), None::<&gio::Cancellable>, move |res| {
                let Ok(file) = res else {
                    return;
                };
                let Some(path) = file.path() else {
                    st.set_label("Non-path URIs: not implemented yet.");
                    return;
                };
                if let Err(e) = try_load(&path, p_c.as_ref(), &st, &gl_w, &recent_choose, true) {
                    st.set_label(&e);
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
                .comments("mpv with GTK 4 and libadwaita (ToolbarView: seek as bottom bar).")
                .license_type(gtk::License::Gpl30)
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
    quit.connect_activate(glib::clone!(
        #[strong]
        app_q,
        #[strong]
        p_quit,
        #[strong]
        win_q,
        move |_, _| {
            schedule_quit_persist(&app_q, &win_q, &p_quit);
        }
    ));
    app.add_action(&quit);

    app.set_accels_for_action("app.open", &["<Primary>o"]);
    app.set_accels_for_action("app.about", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Primary>q", "q"]);

    {
        let p = player.clone();
        let w = win.clone();
        win.connect_close_request(glib::clone!(
            #[strong]
            app_q,
            #[strong]
            p,
            #[strong]
            w,
            move |_win| {
                schedule_quit_persist(&app_q, &w, &p);
                glib::Propagation::Stop
            }
        ));
    }

    apply_chrome(&win, &root, &status, &gl_area, &fs_overlay);

    win.present();
}
