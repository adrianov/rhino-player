use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::glib::prelude::ObjectExt;
use gtk::prelude::{GtkWindowExt, NativeExt, WidgetExt};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::audio_tracks;
use crate::db;
use crate::format_time;
use crate::history;
use libmpv2::Mpv;
use crate::icons;

use crate::media_probe::{
    card_data_list, local_file_from_mpv, record_playback_for_current, save_cached_thumb, CardData,
};
use crate::mpv_embed::MpvBundle;
use crate::recent_view;
use crate::recent_view::RecentContext;
use crate::sibling_advance;
use crate::theme;

/// Application and icon name ([reverse-DNS] for GTK, desktop, and AppStream).
///
/// [reverse-DNS]: https://developer.gnome.org/documentation/tutorials/application-id.html
pub const APP_ID: &str = "ch.rhino.RhinoPlayer";
const APP_WIN_TITLE: &str = "Rhino Player";

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

fn same_xy(a: f64, b: f64) -> bool {
    (a - b).abs() < COORD_EPS
}

/// State for 3s auto-hide: header [gtk::MenuButton]s delay hiding while open (sound + main menu; audio tracks are inside the sound popover).
struct ChromeBarHide {
    nav: Rc<RefCell<Option<glib::SourceId>>>,
    vol: gtk::MenuButton,
    main: gtk::MenuButton,
    root: adw::ToolbarView,
    gl: gtk::GLArea,
    bar_show: Rc<Cell<bool>>,
    recent: gtk::ScrolledWindow,
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

/// True when the current file looks fully played, so we can drop it from the continue list on switch.
/// Aligns with [media_probe] near-end for 100%: EOF or last ~3s of a known duration.
fn mpv_fully_watched(mpv: &Mpv) -> bool {
    if mpv.get_property::<bool>("eof-reached").unwrap_or(false) {
        return true;
    }
    match (
        mpv.get_property::<f64>("time-pos"),
        mpv.get_property::<f64>("duration"),
    ) {
        (Ok(p), Ok(d)) if p.is_finite() && d > 0.0 => d - p <= 3.0,
        _ => false,
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
    const SUFFIX: &[&str] = &[
        "mp4", "m4v", "mkv", "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "m2ts", "mts",
        "vob", "ogv", "3gp", "3g2", "asf", "ts", "mxf", "f4v", "divx", "xvid", "h264", "h265", "hevc",
        "y4m", "yuv", "nsv", "dvr-ms", "rmp4",
    ];
    let f = gtk::FileFilter::new();
    f.set_name(Some("Video files"));
    f.add_mime_type("video/*");
    for s in SUFFIX {
        f.add_suffix(s);
    }
    f
}

/// Fullscreen and **maximized** are tied so the titlebar restore / unmaximize control matches
/// fullscreen. The **titlebar maximize** action only maximizes first; `connect_maximized_notify` then
/// calls `fullscreen()` so the same control always ends in true fullscreen.
fn toggle_fullscreen(
    win: &adw::ApplicationWindow,
    fs_restore: &RefCell<Option<(i32, i32)>>,
    last_unmax: &RefCell<(i32, i32)>,
) {
    if win.is_fullscreen() {
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
) {
    root.set_extend_content_to_top_edge(true);
    root.set_extend_content_to_bottom_edge(true);
    let show = if recent.is_visible() { true } else { bar_show.get() };
    root.set_reveal_top_bars(show);
    root.set_reveal_bottom_bars(show);
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

fn schedule_bars_autohide(ctx: Rc<ChromeBarHide>) {
    replace_timeout(Rc::clone(&ctx.nav), {
        let ctx2 = Rc::clone(&ctx);
        move || {
            if ctx2.vol.is_active() || ctx2.main.is_active() {
                schedule_bars_autohide(Rc::clone(&ctx2));
            } else {
                ctx2.bar_show.set(false);
                apply_chrome(&ctx2.root, &ctx2.gl, &ctx2.bar_show, &ctx2.recent);
                ctx2
                    .squelch
                    .set(Some(Instant::now() + LAYOUT_SQUELCH));
            }
        }
    });
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
        icons::register_hicolor_from_manifest();
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
    let mut g = player.borrow_mut();
    let b = g.as_mut().ok_or("Player not ready. Wait for GL init.")?;
    let prev = local_file_from_mpv(&b.mpv).or_else(|| o.last_path.borrow().clone());
    let drop_from_history = prev
        .as_ref()
        .is_some_and(|p| !same_open_target(p, path) && mpv_fully_watched(&b.mpv));
    if let Err(e) = b.load_file_path(path) {
        eprintln!("[rhino] try_load: loadfile failed: {e}");
        return Err(e);
    }
    eprintln!("[rhino] try_load: loadfile ok");
    if drop_from_history {
        if let Some(p) = prev {
            history::remove(&p);
        }
    }
    *o.last_path.borrow_mut() = std::fs::canonicalize(path).ok();
    if record {
        history::record(path);
    }
    let t = title_for_open_path(path);
    win.set_title(Some(t.as_str()));
    recent_layer.set_visible(false);
    if let Some(f) = o.on_start.as_ref() {
        f();
    }
    gl.queue_render();
    if play_on_start {
        let _ = b.mpv.set_property("pause", false);
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
    sync_window_aspect_from_mpv(&b.mpv, o.win_aspect.as_ref());
    schedule_window_fit_h_video(Rc::clone(player), win.clone());
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
    drop(g);
    seof.done.set(true);
    if let Some(np) = next {
        let o = LoadOpts {
            record: true,
            play_on_start: true,
            last_path: Rc::clone(last_path),
            on_start: Some(Rc::clone(on_start)),
            win_aspect: Rc::clone(&win_aspect),
        };
        if let Err(e) = try_load(&np, player, win, gl, recent, &o) {
            eprintln!("[rhino] sibling advance: {e}");
            seof.done.set(false);
            seof.stall.set((0.0, 0));
        }
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

/// Rebuild the continue row from [history] after a remove or undo.
fn reflow_continue_cards(
    row: &gtk::Box,
    recent: &gtk::ScrolledWindow,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    rbf: &Rc<RefCell<Option<Rc<RecentContext>>>>,
) {
    let r: Vec<PathBuf> = history::load().into_iter().take(5).collect();
    if r.is_empty() {
        recent.set_visible(false);
        return;
    }
    recent.set_visible(true);
    let v: Vec<CardData> = card_data_list(&r);
    recent_view::fill_row(row, v, on_open.clone(), on_remove.clone());
    let n = recent_view::ensure_recent_backfill(rbf, row, on_open, on_remove);
    recent_view::schedule_thumb_backfill(n, r);
}

/// Shared handles for leaving playback and repainting the recent grid (Escape path).
struct BackToBrowseCtx {
    player: Rc<RefCell<Option<MpvBundle>>>,
    on_open: RcPathFn,
    on_remove: RcPathFn,
    recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    sibling_seof: Rc<SiblingEofState>,
    win_aspect: Rc<Cell<Option<f64>>>,
    /// Show bars; cancel auto-hide. Call after [gtk::ScrolledWindow::set_visible] for the grid.
    on_browse: Rc<dyn Fn()>,
    undo_revealer: gtk::Revealer,
    last_removed: Rc<RefCell<Option<PathBuf>>>,
    undo_hide: Rc<RefCell<Option<glib::SourceId>>>,
}

/// Show the sheet immediately; mpv/DB/grid/stop on LOW-priority idles (after a frame paints).
fn back_to_browse(
    c: &BackToBrowseCtx,
    win: &impl IsA<gtk::Window>,
    gl: &gtk::GLArea,
    recent: &gtk::ScrolledWindow,
    row: &gtk::Box,
) {
    c.undo_revealer.set_reveal_child(false);
    if let Some(id) = c.undo_hide.borrow_mut().take() {
        id.remove();
    }
    *c.last_removed.borrow_mut() = None;
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
    let p_write = c.player.clone();
    let row2 = row.clone();
    let op2 = c.on_open.clone();
    let osl2 = c.on_remove.clone();
    let paths2 = paths;
    let rbb = c.recent_backfill.clone();
    let _ = glib::source::idle_add_local_once(move || {
        if let Some(b) = p_write.borrow().as_ref() {
            b.write_resume_snapshot();
            // DB row for % in the grid; avoid `persist_on_quit` here — its screenshot blocks
            // the main loop until after the sheet is filled (Escape feels instant).
            record_playback_for_current(&b.mpv);
        }
        let p3 = p_write.clone();
        let rbb2 = rbb.clone();
        let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
            let v: Vec<CardData> = card_data_list(&paths2);
            recent_view::fill_row(&row2, v, op2.clone(), osl2.clone());
            let n =
                recent_view::ensure_recent_backfill(&rbb2, &row2, op2.clone(), osl2.clone());
            recent_view::schedule_thumb_backfill(n, paths2.clone());
            let p_thumb = p3.clone();
            let p_stop = p3.clone();
            let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
                if let Some(b) = p_thumb.borrow().as_ref() {
                    save_cached_thumb(&b.mpv);
                }
                let p_end = p_stop.clone();
                let _ = glib::source::idle_add_local_full(glib::Priority::LOW, move || {
                    if let Some(b) = p_end.borrow().as_ref() {
                        b.stop_playback();
                    }
                    glib::ControlFlow::Break
                });
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
    let sibling_seof = Rc::new(SiblingEofState {
        done: Cell::new(false),
        stall: Cell::new((0.0, 0u8)),
    });
    let fs_restore = Rc::new(RefCell::new(None::<(i32, i32)>));
    let last_unmax = Rc::new(RefCell::new((WIN_INIT_W, WIN_INIT_H)));
    let win_aspect = Rc::new(Cell::new(None::<f64>));
    let aspect_resize_end_deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let aspect_resize_wired = Rc::new(Cell::new(false));

    let root = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    header.add_css_class("rpb-header");
    let play_pause = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_pause.add_css_class("flat");
    play_pause.add_css_class("rpb-play");
    play_pause.set_tooltip_text(Some("Play (Space)"));
    play_pause.set_sensitive(false);
    let menu = gio::Menu::new();
    menu.append(Some("Open video…"), Some("app.open"));
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
    sound_col.set_margin_start(8);
    sound_col.set_margin_end(8);
    sound_col.set_margin_top(8);
    sound_col.set_margin_bottom(6);
    sound_col.append(&vol_row);
    sound_col.append(&audio_tracks_section);
    let vol_pop = gtk::Popover::new();
    vol_pop.set_child(Some(&sound_col));
    let vol_menu = gtk::MenuButton::new();
    vol_menu.set_icon_name("audio-volume-high-symbolic");
    vol_menu.set_tooltip_text(Some("Sound: volume and audio track"));
    vol_menu.set_popover(Some(&vol_pop));
    vol_menu.add_css_class("flat");

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_tooltip_text(Some("Main menu"));
    menu_btn.set_menu_model(Some(&menu));
    header.pack_end(&menu_btn);
    header.pack_end(&vol_menu);

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

    let dbl = gtk::GestureClick::new();
    dbl.set_button(gtk::gdk::BUTTON_PRIMARY);
    {
        let win_fs = win.clone();
        let fr = fs_restore.clone();
        let lu = last_unmax.clone();
        dbl.connect_pressed(move |_, n_press, _, _| {
            if n_press == 2 {
                toggle_fullscreen(&win_fs, &fr, &lu);
            }
        });
    }
    gl_area.add_controller(dbl);

    let rpp = gtk::GestureClick::new();
    rpp.set_button(gtk::gdk::BUTTON_SECONDARY);
    rpp.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let p_btn = player.clone();
        let glbtn = gl_area.clone();
        rpp.connect_pressed(move |_, n_press, _, _| {
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
    bottom.append(&play_pause);
    bottom.append(&time_left);
    bottom.append(&seek);
    bottom.append(&time_right);

    let ovl = gtk::Overlay::new();
    ovl.add_css_class("rp-stack");
    ovl.add_css_class("rp-page-stack");
    ovl.set_child(Some(&gl_area));

    let (recent_scrl, flow_recent, undo_revealer, undo_btn) = recent_view::new_scroll();
    recent_scrl.set_vexpand(true);
    recent_scrl.set_hexpand(true);
    recent_scrl.set_halign(gtk::Align::Fill);
    recent_scrl.set_valign(gtk::Align::Fill);
    ovl.add_overlay(&recent_scrl);

    let want_recent = startup.is_none() && !history::load().is_empty();
    recent_scrl.set_visible(want_recent);

    let ch_hide = Rc::new(ChromeBarHide {
        nav: nav_t.clone(),
        vol: vol_menu.clone(),
        main: menu_btn.clone(),
        root: root.clone(),
        gl: gl_area.clone(),
        bar_show: bar_show.clone(),
        recent: recent_scrl.clone(),
        squelch: motion_squelch.clone(),
    });

    let on_video_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let chh = Rc::clone(&ch_hide);
        Rc::new(move || {
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent);
            schedule_bars_autohide(Rc::clone(&chh));
        })
    };
    {
        let ch = Rc::clone(&ch_hide);
        let h = Rc::new(move || {
            let any = ch.vol.is_active() || ch.main.is_active();
            if any {
                if let Some(id) = ch.nav.borrow_mut().take() {
                    id.remove();
                }
                ch.bar_show.set(true);
                apply_chrome(&ch.root, &ch.gl, &ch.bar_show, &ch.recent);
            } else {
                schedule_bars_autohide(Rc::clone(&ch));
            }
        });
        let h1 = Rc::clone(&h);
        let h2 = Rc::clone(&h);
        vol_menu.connect_active_notify(move |_| h1());
        menu_btn.connect_active_notify(move |_| h2());
    }
    let browse_chrome: Rc<dyn Fn()> = {
        let root = root.clone();
        let gl = gl_area.clone();
        let b = bar_show.clone();
        let recent = recent_scrl.clone();
        let nav = nav_t.clone();
        Rc::new(move || {
            if let Some(id) = nav.borrow_mut().take() {
                id.remove();
            }
            b.set(true);
            apply_chrome(&root, &gl, &b, &recent);
        })
    };
    let on_open_vid = on_video_chrome.clone();
    let p_openr = player.clone();
    let win_menu = win.clone();
    let gl_op = gl_area.clone();
    let recent_on_top = recent_scrl.clone();
    let last_open = last_path.clone();
    let wa_on = Rc::clone(&win_aspect);
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
                on_start: Some(Rc::clone(&on_open_vid)),
                win_aspect: wa_on.clone(),
            },
        ) {
            eprintln!("[rhino] on_open: try_load error: {e}");
        }
    });

    let recent_backfill: Rc<RefCell<Option<Rc<RecentContext>>>> = Rc::new(RefCell::new(None));
    {
        let rb = recent_backfill.clone();
        recent_scrl.connect_destroy(move |_| {
            if let Some(ctx) = rb.borrow_mut().take() {
                ctx.shutdown();
            }
        });
    }

    let last_removed = Rc::new(RefCell::new(None::<PathBuf>));
    let undo_hide = Rc::new(RefCell::new(None::<glib::SourceId>));
    let on_remove_cell: Rc<RefCell<Option<RcPathFn>>> = Rc::new(RefCell::new(None));
    let fr_sl = flow_recent.clone();
    let recent_rm = recent_scrl.clone();
    let op_s = on_open.clone();
    let rbf_rm = recent_backfill.clone();
    let lr_rm = last_removed.clone();
    let urev_rm = undo_revealer.clone();
    let uto_rm = undo_hide.clone();
    let cell_rm = on_remove_cell.clone();
    let on_remove: RcPathFn = Rc::new(move |path: &Path| {
        if let Some(id) = uto_rm.borrow_mut().take() {
            id.remove();
        }
        history::remove(path);
        *lr_rm.borrow_mut() = Some(path.to_path_buf());
        urev_rm.set_reveal_child(true);
        let f = cell_rm
            .borrow()
            .as_ref()
            .expect("on_remove not wired")
            .clone();
        reflow_continue_cards(&fr_sl, &recent_rm, op_s.clone(), f, &rbf_rm);
        *uto_rm.borrow_mut() = Some(glib::timeout_add_local(
            Duration::from_secs(8),
            glib::clone!(
                #[strong]
                urev_rm,
                #[strong]
                lr_rm,
                #[strong]
                uto_rm,
                move || {
                    urev_rm.set_reveal_child(false);
                    lr_rm.borrow_mut().take();
                    uto_rm.borrow_mut().take();
                    glib::ControlFlow::Break
                }
            ),
        ));
    });
    *on_remove_cell.borrow_mut() = Some(on_remove.clone());

    {
        let fr_u = flow_recent.clone();
        let rec_u = recent_scrl.clone();
        let op_u = on_open.clone();
        let rbf_u = recent_backfill.clone();
        let lr_u = last_removed.clone();
        let urev_u = undo_revealer.clone();
        let uto_u = undo_hide.clone();
        let cell_u = on_remove_cell.clone();
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
            lr_u,
            #[strong]
            urev_u,
            #[strong]
            uto_u,
            #[strong]
            cell_u,
            move |_| {
                if let Some(id) = uto_u.borrow_mut().take() {
                    id.remove();
                }
                let Some(pb) = lr_u.borrow_mut().take() else {
                    return;
                };
                history::record(&pb);
                urev_u.set_reveal_child(false);
                rec_u.set_visible(true);
                let f = cell_u
                    .borrow()
                    .as_ref()
                    .expect("on_remove not wired")
                    .clone();
                reflow_continue_cards(&fr_u, &rec_u, op_u.clone(), f, &rbf_u);
            }
        ));
    }

    if want_recent {
        let paths5: Vec<PathBuf> = history::load().into_iter().take(5).collect();
        recent_view::fill_idle(
            &flow_recent,
            paths5,
            on_open.clone(),
            on_remove.clone(),
            recent_backfill.clone(),
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
        let b = bar_show.clone();
        let nav = nav_t.clone();
        let sq = motion_squelch.clone();
        let lcap = last_cap_xy.clone();
        let lgl = last_gl_xy.clone();
        let fr = fs_restore.clone();
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
            }
            apply_chrome(&root_fs, &gl_fs, &b, &recent_fs);
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
        win.connect_maximized_notify(move |w| {
            if !w.is_maximized() && !w.is_fullscreen() {
                *lu.borrow_mut() = win_normal_size(w);
            } else if !w.is_maximized() && w.is_fullscreen() {
                w.unfullscreen();
            } else if w.is_maximized() && !w.is_fullscreen() {
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
                apply_chrome(&root_c, &gl_c, &b, &recent_c);
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
        let rbf_esc = recent_backfill.clone();
        let last_esc = last_path.clone();
        let seof_esc = sibling_seof.clone();
        let browse_esc = browse_chrome.clone();
        let fr_key = fs_restore.clone();
        let lu_key = last_unmax.clone();
        let wa_esc = win_aspect.clone();
        let urev_k = undo_revealer.clone();
        let lr_k = last_removed.clone();
        let uto_k = undo_hide.clone();
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
                    &BackToBrowseCtx {
                        player: p.clone(),
                        on_open: op_esc.clone(),
                        on_remove: rem_esc.clone(),
                        recent_backfill: rbf_esc.clone(),
                        last_path: last_esc.clone(),
                        sibling_seof: seof_esc.clone(),
                        win_aspect: wa_esc.clone(),
                        on_browse: browse_esc.clone(),
                        undo_revealer: urev_k.clone(),
                        last_removed: lr_k.clone(),
                        undo_hide: uto_k.clone(),
                    },
                    &win_key,
                    &gl_esc,
                    &recent_esc,
                    &flow_esc,
                );
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
                toggle_fullscreen(&win_key, &fr_key, &lu_key);
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
    let win_rz = win.clone();
    let gl_rz = gl_area.clone();
    let recent_rz = recent_scrl.clone();
    let last_rz = last_path.clone();
    let on_vid_rz = on_video_chrome.clone();
    let st_path = startup;
    let wa_st = Rc::clone(&win_aspect);
    gl_area.connect_realize(move |area| {
        area.make_current();
        match MpvBundle::new(area) {
            Ok(b) => {
                let (av, am) = db::load_audio();
                let _ = b.mpv.set_property("volume", av);
                let _ = b.mpv.set_property("mute", am);
                *p_realize.borrow_mut() = Some(b);
                if let Some(bundle) = p_realize.borrow_mut().as_mut() {
                    let _ = bundle.mpv.disable_deprecated_events();
                }
                if let Some(ref p) = st_path {
                    if let Err(e) = try_load(
                        p,
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
                let _ = b.mpv.set_property("time-pos", r.value());
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
                );
                let Some(tl) = tw_l.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(tr) = tw_r.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let g = p_poll.borrow();
                let Some(pl) = g.as_ref() else {
                    if let Some(pp) = ppw.upgrade() {
                        pp.set_sensitive(false);
                        pp.set_icon_name("media-playback-start-symbolic");
                        pp.set_tooltip_text(Some("No media"));
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
                .logo_icon_name(APP_ID)
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

    apply_chrome(&root, &gl_area, &bar_show, &recent_scrl);

    win.present();
}
