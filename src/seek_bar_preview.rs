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
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(120);
const VO_PUMP_STEP: Duration = Duration::from_millis(33);

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

fn set_popover_non_modal(pop: &gtk::Popover) {
    if pop.find_property("modal").is_some() {
        pop.set_property("modal", false);
    }
}

fn point_popover_at(pop: &gtk::Popover, seek: &gtk::Scale, x: f64) {
    let w = f64::from(seek.width().max(1));
    let x_cl = x.clamp(2.0, w - 2.0) as i32;
    let r = gtk::gdk::Rectangle::new(x_cl, -6, 1, 1);
    pop.set_pointing_to(Some(&r));
}

fn set_preview_size(gl: &gtk::GLArea, seek: &gtk::Scale, player: &Rc<RefCell<Option<MpvBundle>>>) {
    let (dw, dh) = player
        .borrow()
        .as_ref()
        .map(|b| {
            let dw = b.mpv.get_property::<i64>("dwidth").unwrap_or(0) as i32;
            let dh = b.mpv.get_property::<i64>("dheight").unwrap_or(0) as i32;
            (dw.max(1), dh.max(1))
        })
        .unwrap_or((1920, 1080));
    let (req_w, req_h) = preview_size(dw, dh, preview_px(seek.width()));
    gl.set_size_request(req_w, req_h);
}

fn start_vo_pump(
    gl: &gtk::GLArea,
    preview: Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: Rc<RefCell<Option<glib::SourceId>>>,
    serial: Rc<Cell<u64>>,
    run_id: u64,
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
    let id = glib::source::timeout_add_local_full(VO_PUMP_STEP, glib::Priority::LOW, move || {
        if serial.get() != run_id {
            return glib::ControlFlow::Break;
        }
        n2.set(n2.get() + 1);
        if n2.get() > 90 {
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
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    *pump.borrow_mut() = Some(id);
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
    let serial = Rc::new(Cell::new(0u64));
    let loaded_path = Rc::new(RefCell::new(None::<PathBuf>));

    let pop = gtk::Popover::new();
    pop.set_autohide(false);
    pop.set_has_arrow(false);
    set_popover_non_modal(&pop);
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
        serial,
        #[strong]
        loaded_path,
        move |_, x, y| {
            if st.last_xy.borrow().is_some_and(|p| p == (x, y)) {
                return;
            }
            serial.set(serial.get().wrapping_add(1));
            let run_id = serial.get();
            *st.last_xy.borrow_mut() = Some((x, y));
            let w = f64::from(st.seek.width().max(1));
            let dur = st.seek_adj.upper();
            if dur <= 0.0 {
                return;
            }
            let t = (x / w).clamp(0.0, 1.0) * dur;
            st.hover_t.set(t);
            st.time_lbl.set_text(&format_time(t));
            set_preview_size(&gl, &st.seek, &st.player);

            if let Some(sid) = st.deb.borrow_mut().take() {
                sid.remove();
            }
            if let Some(sid) = pump.borrow_mut().take() {
                sid.remove();
            }
            if !st.enabled.get() {
                gl.set_visible(false);
                st.pop.popdown();
                return;
            }
            let path = st.player.borrow().as_ref().and_then(|b| {
                local_file_from_mpv(&b.mpv).or_else(|| st.last_path.borrow().clone())
            });
            let path_ok = path.as_ref().is_some_and(|p| p.is_file());
            if !path_ok {
                gl.set_visible(false);
                st.pop.popdown();
                return;
            }
            if st.pop.is_visible() {
                point_popover_at(&st.pop, &st.seek, x);
            }
            let st2 = Rc::clone(&st);
            let gl2 = gl.clone();
            let pr2 = Rc::clone(&preview);
            let pmp = Rc::clone(&pump);
            let serial2 = Rc::clone(&serial);
            let lp2 = Rc::clone(&loaded_path);
            let tries = Rc::new(Cell::new(0i32));
            let tries2 = Rc::clone(&tries);
            *st.deb.borrow_mut() = Some(glib::source::timeout_add_local_full(
                PREVIEW_DEBOUNCE,
                glib::Priority::LOW,
                move || {
                    let _ = st2.deb.borrow_mut().take();
                    if serial2.get() != run_id {
                        return glib::ControlFlow::Break;
                    }
                    if !st2.enabled.get() {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    }
                    let p = st2.player.borrow().as_ref().and_then(|b| {
                        local_file_from_mpv(&b.mpv).or_else(|| st2.last_path.borrow().clone())
                    });
                    let Some(pth) = p else {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    };
                    if !pth.is_file() {
                        gl2.set_visible(false);
                        return glib::ControlFlow::Break;
                    }
                    if let Some((x, _)) = *st2.last_xy.borrow() {
                        point_popover_at(&st2.pop, &st2.seek, x);
                    }
                    gl2.set_visible(true);
                    if !st2.pop.is_visible() {
                        st2.pop.popup();
                    }
                    if pr2.borrow().is_none() {
                        tries2.set(tries2.get() + 1);
                        if tries2.get() < 20 {
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
                    let t = (st2.hover_t.get()).clamp(0.0, (mpv_d - 0.01).max(0.0));
                    let canon = std::fs::canonicalize(&pth).unwrap_or(pth);
                    {
                        let mut g = pr2.borrow_mut();
                        let Some(pr) = g.as_mut() else {
                            gl2.set_visible(false);
                            return glib::ControlFlow::Break;
                        };
                        let need_load = lp2.borrow().as_ref().map(|c| c != &canon).unwrap_or(true);
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
                                Rc::clone(&serial2),
                                run_id,
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
                            gl2.set_visible(true);
                            gl2.queue_render();
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
        serial,
        move |_| {
            serial.set(serial.get().wrapping_add(1));
            if let Some(s) = st.deb.borrow_mut().take() {
                s.remove();
            }
            if let Some(s) = pump.borrow_mut().take() {
                s.remove();
            }
            st.pop.popdown();
            gl.set_visible(false);
        }
    ));
    seek.add_controller(mot);

    st
}
