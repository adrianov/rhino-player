//! Seek bar hover: a **second** [libmpv] with [vo=libmpv] in a small [`gtk::GLArea`]
//! (same OpenGL path as the main [crate::mpv_embed::MpvBundle] — not `screenshot-raw`).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;

use crate::format_time;
use crate::media_probe::local_file_from_mpv;
use crate::mpv_embed::{set_preview_tracks, MpvBundle, MpvPreviewGl};

const PREVIEW_MIN_PX: i32 = 180;
const PREVIEW_MAX_PX: i32 = 320;

pub struct SeekPreviewState {
    deb: Rc<RefCell<Option<glib::SourceId>>>,
    last_xy: Rc<RefCell<Option<(f64, f64)>>>,
    hover_t: Rc<Cell<f64>>,
    pop: gtk::Popover,
    time_lbl: gtk::Label,
    pub enabled: Rc<Cell<bool>>,
    seek: gtk::Scale,
    seek_adj: gtk::Adjustment,
    player: Rc<RefCell<Option<MpvBundle>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
}

impl SeekPreviewState {
    /// No-op (kept so [crate::app] transport tick wiring stays stable).
    pub fn on_tick(&self) {}
}

fn preview_px(seek_w: i32) -> i32 {
    ((f64::from(seek_w) * 0.16).round() as i32).clamp(PREVIEW_MIN_PX, PREVIEW_MAX_PX)
}

fn preview_size(dw: i32, dh: i32, long_edge: i32) -> (i32, i32) {
    let (w, h) = if dw >= dh {
        let h = (long_edge as f64 * dh as f64 / dw.max(1) as f64) as i32;
        (long_edge, h.max(1))
    } else {
        let w = (long_edge as f64 * dw as f64 / dh.max(1) as f64) as i32;
        (w.max(1), long_edge)
    };
    (w, h)
}

fn start_vo_pump(
    gl: &gtk::GLArea,
    preview: Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: Rc<RefCell<Option<glib::SourceId>>>,
    exact: Rc<RefCell<Option<glib::SourceId>>>,
    seek_sec: f64,
) {
    if let Some(s) = pump.borrow_mut().take() {
        s.remove();
    }
    let t_s = format!("{seek_sec:.3}");
    let gl2 = gl.clone();
    let pr2 = Rc::clone(&preview);
    let pump2 = Rc::clone(&pump);
    let n = Rc::new(Cell::new(0i32));
    let n2 = Rc::clone(&n);
    let id = glib::source::timeout_add_local(Duration::from_millis(16), move || {
        n2.set(n2.get() + 1);
        if n2.get() > 200 {
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }
        let done = {
            let mut p = pr2.borrow_mut();
            if let Some(pr) = p.as_mut() {
                while pr.mpv.wait_event(0.0).is_some() {}
                if pr.mpv.get_property::<bool>("vo-configured") == Ok(true) {
                    let _ = pr
                        .mpv
                        .command("seek", &[t_s.as_str(), "absolute+keyframes"]);
                    true
                } else {
                    false
                }
            } else {
                *pump2.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }
        };
        if done {
            gl2.queue_render();
            schedule_exact_seek(Rc::clone(&pr2), Rc::clone(&exact), seek_sec);
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    *pump.borrow_mut() = Some(id);
}

fn schedule_exact_seek(
    preview: Rc<RefCell<Option<MpvPreviewGl>>>,
    exact: Rc<RefCell<Option<glib::SourceId>>>,
    seek_sec: f64,
) {
    if let Some(s) = exact.borrow_mut().take() {
        s.remove();
    }
    let t_s = format!("{seek_sec:.3}");
    let ex2 = Rc::clone(&exact);
    let id = glib::source::timeout_add_local(Duration::from_millis(160), move || {
        if let Some(pr) = preview.borrow_mut().as_mut() {
            let _ = pr.mpv.command("seek", &[t_s.as_str(), "absolute+exact"]);
        }
        *ex2.borrow_mut() = None;
        glib::ControlFlow::Break
    });
    *exact.borrow_mut() = Some(id);
}

pub fn connect(
    seek: &gtk::Scale,
    seek_adj: &gtk::Adjustment,
    player: Rc<RefCell<Option<MpvBundle>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    enabled: Rc<Cell<bool>>,
) -> Rc<SeekPreviewState> {
    let last_xy = Rc::new(RefCell::new(None::<(f64, f64)>));
    let deb = Rc::new(RefCell::new(None::<glib::SourceId>));
    let hover_t = Rc::new(Cell::new(0.0f64));
    let preview = Rc::new(RefCell::new(None::<MpvPreviewGl>));
    let pump = Rc::new(RefCell::new(None::<glib::SourceId>));
    let exact = Rc::new(RefCell::new(None::<glib::SourceId>));
    let loaded_path = Rc::new(RefCell::new(None::<PathBuf>));

    let pop = gtk::Popover::new();
    pop.set_autohide(false);
    pop.set_has_arrow(false);
    pop.set_position(gtk::PositionType::Top);
    pop.set_offset(0, -8);
    pop.set_parent(seek);
    pop.add_css_class("rp-seek-popover");

    let frame = gtk::Frame::new(None::<&str>);
    frame.add_css_class("rp-seek-thumb-frame");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);

    let gl = gtk::GLArea::new();
    gl.set_valign(gtk::Align::Start);
    gl.set_halign(gtk::Align::Center);
    gl.set_size_request(180, 101);
    gl.set_width_request(180);
    gl.set_height_request(101);
    gl.set_auto_render(false);
    gl.set_has_stencil_buffer(false);
    gl.set_has_depth_buffer(false);
    gl.set_visible(false);

    let time_lbl = gtk::Label::new(None::<&str>);
    time_lbl.add_css_class("rp-seek-thumb-time");
    time_lbl.add_css_class("numeric");
    time_lbl.set_xalign(0.5);

    body.append(&gl);
    body.append(&time_lbl);
    frame.set_child(Some(&body));
    pop.set_child(Some(&frame));

    let pr_realize = Rc::clone(&preview);
    gl.connect_realize(move |a| {
        a.make_current();
        match MpvPreviewGl::new(a) {
            Ok(p) => {
                *pr_realize.borrow_mut() = Some(p);
            }
            Err(e) => eprintln!("[rhino] seek preview GL/mpv: {e}"),
        }
    });

    let pr_draw = Rc::clone(&preview);
    let gl_draw = gl.clone();
    gl.connect_render(move |area, _| {
        area.make_current();
        if let Some(p) = pr_draw.borrow().as_ref() {
            p.draw(&gl_draw);
        }
        glib::Propagation::Stop
    });

    let st = Rc::new(SeekPreviewState {
        deb: Rc::clone(&deb),
        last_xy: Rc::clone(&last_xy),
        hover_t: Rc::clone(&hover_t),
        pop: pop.clone(),
        time_lbl: time_lbl.clone(),
        enabled: Rc::clone(&enabled),
        seek: seek.clone(),
        seek_adj: seek_adj.clone(),
        player: Rc::clone(&player),
        last_path: Rc::clone(&last_path),
    });

    let mot = gtk::EventControllerMotion::new();
    mot.connect_motion(glib::clone!(
        #[strong]
        st,
        #[strong]
        gl,
        #[strong]
        preview,
        #[strong]
        pump,
        #[strong]
        exact,
        #[strong]
        loaded_path,
        move |_, x, y| {
            if st.last_xy.borrow().is_some_and(|p| p == (x, y)) {
                return;
            }
            *st.last_xy.borrow_mut() = Some((x, y));
            let w = f64::from(st.seek.width().max(1));
            let dur = st.seek_adj.upper();
            if dur <= 0.0 {
                return;
            }
            let t = (x / w).clamp(0.0, 1.0) * dur;
            st.hover_t.set(t);
            st.time_lbl.set_text(&format_time(t));

            let x_cl = x.clamp(2.0, w - 2.0) as i32;
            let r = gtk::gdk::Rectangle::new(x_cl, -6, 1, 1);
            st.pop.set_pointing_to(Some(&r));
            st.pop.popup();
            if let Some(sid) = st.deb.borrow_mut().take() {
                sid.remove();
            }
            if let Some(sid) = exact.borrow_mut().take() {
                sid.remove();
            }
            if !st.enabled.get() {
                gl.set_visible(false);
                return;
            }
            let path = st
                .player
                .borrow()
                .as_ref()
                .and_then(|b| {
                    local_file_from_mpv(&b.mpv).or_else(|| st.last_path.borrow().clone())
                });
            let path_ok = path.as_ref().is_some_and(|p| p.is_file());
            if !path_ok {
                gl.set_visible(false);
                return;
            }
            gl.set_visible(true);
            let st2 = Rc::clone(&st);
            let gl2 = gl.clone();
            let pr2 = Rc::clone(&preview);
            let pmp = Rc::clone(&pump);
            let ex2 = Rc::clone(&exact);
            let lp2 = Rc::clone(&loaded_path);
            let tries = Rc::new(Cell::new(0i32));
            let tries2 = Rc::clone(&tries);
            *st.deb.borrow_mut() = Some(glib::source::timeout_add_local(
                Duration::from_millis(70),
                move || {
                    let _ = st2.deb.borrow_mut().take();
                    if !st2.enabled.get() {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    }
                    let p = st2
                        .player
                        .borrow()
                        .as_ref()
                        .and_then(|b| {
                            local_file_from_mpv(&b.mpv)
                                .or_else(|| st2.last_path.borrow().clone())
                        });
                    let Some(pth) = p else {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    };
                    if !pth.is_file() {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    }
                    if pr2.borrow().is_none() {
                        tries2.set(tries2.get() + 1);
                        if tries2.get() < 20 {
                            gl2.set_visible(true);
                            return glib::ControlFlow::Continue;
                        }
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    }
                    let up = st2.seek_adj.upper();
                    let mpv_d = st2
                        .player
                        .borrow()
                        .as_ref()
                        .and_then(|b| b.mpv.get_property::<f64>("duration").ok())
                        .filter(|d| d.is_finite() && *d > 0.0)
                        .unwrap_or(up);
                    let t = (st2.hover_t.get())
                        .clamp(0.0, (mpv_d - 0.01).max(0.0));
                    let (dw, dh) = st2
                        .player
                        .borrow()
                        .as_ref()
                        .map(|b| {
                            let dw = b.mpv.get_property::<i64>("dwidth").unwrap_or(0) as i32;
                            let dh = b.mpv.get_property::<i64>("dheight").unwrap_or(0) as i32;
                            (dw, dh)
                        })
                        .unwrap_or((0, 0));
                    let dw = if dw > 0 { dw } else { 1920 };
                    let dh = if dh > 0 { dh } else { 1080 };
                    let long_edge = preview_px(st2.seek.width());
                    let (req_w, req_h) = preview_size(dw, dh, long_edge);
                    gl2.set_size_request(req_w, req_h);
                    let canon = std::fs::canonicalize(&pth).unwrap_or(pth);
                    {
                        let mut g = pr2.borrow_mut();
                        let Some(pr) = g.as_mut() else {
                            gl2.set_visible(false);
                            return glib::ControlFlow::Break;
                        };
                        let need_load = lp2
                            .borrow()
                            .as_ref()
                            .map(|c| c != &canon)
                            .unwrap_or(true);
                        if need_load {
                            *lp2.borrow_mut() = Some(canon.clone());
                            let s = match canon.to_str() {
                                Some(s) => s,
                                None => {
                                    gl2.set_visible(false);
                                    return glib::ControlFlow::Break;
                                }
                            };
                            if pr.mpv.command("loadfile", &[s, "replace"]).is_err() {
                                gl2.set_visible(false);
                                return glib::ControlFlow::Break;
                            }
                            set_preview_tracks(&pr.mpv);
                            gl2.set_visible(true);
                            start_vo_pump(
                                &gl2,
                                Rc::clone(&pr2),
                                Rc::clone(&pmp),
                                Rc::clone(&ex2),
                                t,
                            );
                        } else {
                            set_preview_tracks(&pr.mpv);
                            let t_s = format!("{t:.3}");
                            if pr
                                .mpv
                                .command("seek", &[t_s.as_str(), "absolute+keyframes"])
                                .is_err()
                            {
                                gl2.set_visible(false);
                                return glib::ControlFlow::Break;
                            }
                            for _ in 0..3 {
                                while pr.mpv.wait_event(0.0).is_some() {}
                            }
                            gl2.set_visible(true);
                            gl2.queue_render();
                            schedule_exact_seek(Rc::clone(&pr2), Rc::clone(&ex2), t);
                        }
                    }
                    glib::ControlFlow::Break
                },
            ));
        }
    ));
    mot.connect_leave(glib::clone!(
        #[strong]
        st,
        #[strong]
        gl,
        #[strong]
        pump,
        #[strong]
        exact,
        move |_| {
            if let Some(s) = st.deb.borrow_mut().take() {
                s.remove();
            }
            if let Some(s) = pump.borrow_mut().take() {
                s.remove();
            }
            if let Some(s) = exact.borrow_mut().take() {
                s.remove();
            }
            st.pop.popdown();
            gl.set_visible(false);
        }
    ));
    seek.add_controller(mot);

    st
}
