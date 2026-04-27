
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

