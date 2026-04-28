const PREVIEW_MIN_PX: i32 = 180;
const PREVIEW_MAX_PX: i32 = 320;
const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(120);
const VO_PUMP_STEP: Duration = Duration::from_millis(33);
const PREVIEW_GAP: i32 = 8;

pub struct SeekPreviewState {
    /// Overlay child — add to the window overlay after [connect], stays on the same
    /// [`GdkSurface`] so there is no compositor surface creation on show/hide.
    pub container: gtk::Frame,
    pub gl: gtk::GLArea,
    pub time_lbl: gtk::Label,
    pub preview: Rc<RefCell<Option<MpvPreviewGl>>>,
    pub pump: Rc<RefCell<Option<glib::SourceId>>>,
    pub serial: Rc<Cell<u64>>,
    pub loaded_path: Rc<RefCell<Option<PathBuf>>>,
    pub enabled: Rc<Cell<bool>>,
    pub seek: gtk::Scale,
    pub seek_adj: gtk::Adjustment,
    pub player: Rc<RefCell<Option<MpvBundle>>>,
    pub last_path: Rc<RefCell<Option<PathBuf>>>,
    pub hover_t: Rc<Cell<f64>>,
    pub last_xy: Rc<RefCell<Option<(f64, f64)>>>,
    pub deb: Rc<RefCell<Option<glib::SourceId>>>,
    pub bottom: gtk::Box,
    pub ovl: gtk::Overlay,
}

impl SeekPreviewState {
    pub(crate) fn show_at(&self, x: f64) {
        // frame: padding 3px + border 1px per side = 8px over gl width; use allocated width when ready.
        let preview_w = self.container.width().max(self.gl.width_request() + 8).max(1) as f64;
        let ovl_w = self.ovl.width().max(1) as f64;
        let cursor_x = self
            .seek
            .compute_point(&self.ovl, &gtk::graphene::Point::new(x as f32, 0.0))
            .map(|p| p.x() as f64)
            .unwrap_or(x);
        let raw = (cursor_x - preview_w / 2.0).round();
        let margin_start = raw.clamp(0.0, (ovl_w - preview_w).max(0.0)) as i32;
        let margin_bottom = self.bottom.height() + PREVIEW_GAP;
        self.container.set_margin_start(margin_start);
        self.container.set_margin_bottom(margin_bottom);
        self.container.set_can_target(false);
        self.container.set_visible(true);
    }

    pub(crate) fn hide(&self) {
        self.container.set_visible(false);
    }
}

fn preview_px(seek_w: i32) -> i32 {
    ((f64::from(seek_w) * 0.16).round() as i32).clamp(PREVIEW_MIN_PX, PREVIEW_MAX_PX)
}

fn preview_size(dw: i32, dh: i32, long_edge: i32) -> (i32, i32) {
    if dw >= dh {
        let h = (long_edge as f64 * dh as f64 / dw.max(1) as f64) as i32;
        (long_edge, h.max(1))
    } else {
        let w = (long_edge as f64 * dw as f64 / dh.max(1) as f64) as i32;
        (w.max(1), long_edge)
    }
}

pub(crate) fn set_preview_size(st: &SeekPreviewState) {
    let (dw, dh) = st
        .player
        .borrow()
        .as_ref()
        .map(|b| {
            let dw = b.mpv.get_property::<i64>("dwidth").unwrap_or(0) as i32;
            let dh = b.mpv.get_property::<i64>("dheight").unwrap_or(0) as i32;
            (dw.max(1), dh.max(1))
        })
        .unwrap_or((1920, 1080));
    let (req_w, req_h) = preview_size(dw, dh, preview_px(st.seek.width()));
    if st.gl.width_request() != req_w || st.gl.height_request() != req_h {
        st.gl.set_size_request(req_w, req_h);
    }
}

pub(crate) fn start_vo_pump(
    gl: &gtk::GLArea,
    preview: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    run_id: u64,
    seek_sec: f64,
) {
    if let Some(s) = pump.borrow_mut().take() {
        s.remove();
    }
    let t_s = format!("{seek_sec:.3}");
    let gl2 = gl.clone();
    let pr2 = Rc::clone(preview);
    let pump2 = Rc::clone(pump);
    let serial2 = Rc::clone(serial);
    let n = Rc::new(Cell::new(0i32));
    let id = glib::source::timeout_add_local_full(VO_PUMP_STEP, glib::Priority::LOW, move || {
        if serial2.get() != run_id {
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }
        n.set(n.get() + 1);
        if n.get() > 90 {
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }
        let mut p = pr2.borrow_mut();
        let Some(pr) = p.as_mut() else {
            *pump2.borrow_mut() = None;
            return glib::ControlFlow::Break;
        };
        while pr.mpv.wait_event(0.0).is_some() {}
        if pr.mpv.get_property::<bool>("vo-configured") == Ok(true) {
            let _ = pr.mpv.command("seek", &[t_s.as_str(), "absolute+keyframes"]);
            gl2.queue_render();
            *pump2.borrow_mut() = None;
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
    *pump.borrow_mut() = Some(id);
}
