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
    pub chapter_lbl: gtk::Label,
    pub time_lbl: gtk::Label,
    pub preview: Rc<RefCell<Option<MpvPreviewGl>>>,
    pub pump: Rc<RefCell<Option<glib::SourceId>>>,
    pub serial: Rc<Cell<u64>>,
    pub loaded_path: Rc<RefCell<Option<PathBuf>>>,
    pub loaded_target: Rc<RefCell<Option<String>>>,
    /// [`PlaybackEntity::db_path`] for the clip loaded in the auxiliary player.
    pub preview_owner_db: Rc<RefCell<Option<PathBuf>>>,
    pub enabled: Rc<Cell<bool>>,
    pub seek: gtk::Scale,
    pub seek_adj: gtk::Adjustment,
    pub player: Rc<RefCell<Option<MpvBundle>>>,
    pub last_path: Rc<RefCell<Option<PathBuf>>>,
    pub chapters: Rc<RefCell<Vec<(f64, String)>>>,
    pub dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    pub hover_t: Rc<Cell<f64>>,
    pub last_xy: Rc<RefCell<Option<(f64, f64)>>>,
    pub deb: Rc<RefCell<Option<glib::SourceId>>>,
    pub bottom: gtk::Box,
    pub ovl: gtk::Overlay,
}

impl SeekPreviewState {
    pub(crate) fn clear_preview_visual(&self) {
        if let Some(pr) = self.preview.borrow().as_ref() {
            pr.clear_framebuffer(&self.gl);
        }
    }

    pub(crate) fn show_at(&self, x: f64) {
        #[cfg(target_os = "macos")]
        let was_visible = self.container.is_visible();
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
        #[cfg(target_os = "macos")]
        if !was_visible {
            macos_compositing::on_open(self);
        }
    }

    pub(crate) fn hide(&self) {
        #[cfg(target_os = "macos")]
        let was_visible = self.container.is_visible();
        self.container.set_visible(false);
        #[cfg(target_os = "macos")]
        if was_visible {
            macos_compositing::on_close();
        }
    }

    /// Main player opened another file — drop cached load target and hide until re-hover.
    pub(crate) fn reset_for_new_media(&self, from: &'static str) {
        crate::preview_debug::info(format!(
            "reset from {from} (prev_target={:?} owner={:?} visible={})",
            self.loaded_target.borrow().as_deref(),
            self.preview_owner_db
                .borrow()
                .as_ref()
                .map(|p| p.display().to_string()),
            self.container.is_visible()
        ));
        self.serial.set(self.serial.get().wrapping_add(1));
        crate::glib_source_drop::drop_glib_source(self.deb.as_ref());
        crate::glib_source_drop::drop_glib_source(self.pump.as_ref());
        *self.loaded_target.borrow_mut() = None;
        *self.loaded_path.borrow_mut() = None;
        *self.preview_owner_db.borrow_mut() = None;
        *self.last_xy.borrow_mut() = None;
        if let Some(pr) = self.preview.borrow().as_ref() {
            reset_preview_player_decode(&pr.mpv);
        }
        self.hide();
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

pub(crate) fn start_preview_frame_pump(
    gl: &gtk::GLArea,
    preview: &Rc<RefCell<Option<MpvPreviewGl>>>,
    pump: &Rc<RefCell<Option<glib::SourceId>>>,
    serial: &Rc<Cell<u64>>,
    run_id: u64,
    load: &str,
    content_dur: f64,
    seek_sec: f64,
    optical: bool,
) {
    crate::glib_source_drop::drop_glib_source(pump.as_ref());
    let gl2 = gl.clone();
    let pr2 = Rc::clone(preview);
    let pump2 = Rc::clone(pump);
    let serial2 = Rc::clone(serial);
    let n = Rc::new(Cell::new(0i32));
    let max_ticks = if optical { 180 } else { 90 };
    crate::preview_debug::info(format!(
        "pump start run={run_id} seek={seek_sec:.2} dur={content_dur:.2} optical={optical} max_ticks={max_ticks}"
    ));
    let load_s = load.to_string();
    let id = glib::source::timeout_add_local_full(VO_PUMP_STEP, glib::Priority::DEFAULT, move || {
        if serial2.get() != run_id {
            *pump2.borrow_mut() = None;
            crate::preview_debug::log(format!("pump run={run_id} cancelled (serial stale)"));
            return glib::ControlFlow::Break;
        }
        n.set(n.get() + 1);
        if n.get() > max_ticks {
            *pump2.borrow_mut() = None;
            let snap = pr2
                .borrow()
                .as_ref()
                .map(|pr| crate::preview_debug::mpv_line(&pr.mpv))
                .unwrap_or_else(|| "no preview".into());
            crate::preview_debug::warn(format!(
                "pump timeout run={run_id} ticks={max_ticks} {snap}"
            ));
            return glib::ControlFlow::Break;
        }
        let mut p = pr2.borrow_mut();
        let Some(pr) = p.as_mut() else {
            *pump2.borrow_mut() = None;
            crate::preview_debug::warn(format!("pump run={run_id} tick={}: no preview player", n.get()));
            return glib::ControlFlow::Break;
        };
        while pr.mpv.wait_event(0.0).is_some() {}
        if pr.mpv.get_property::<bool>("vo-configured") != Ok(true) {
            if n.get() == 1 || n.get() % 15 == 0 {
                crate::preview_debug::log(format!(
                    "pump run={run_id} tick={}: waiting vo-configured ({})",
                    n.get(),
                    crate::preview_debug::mpv_line(&pr.mpv)
                ));
            }
            return glib::ControlFlow::Continue;
        }
        let chapter = std::path::Path::new(&load_s);
        if optical
            && crate::dvd_vob_mpv_probe::is_title_chain_head(chapter)
            && !crate::dvd_vob_timeline::chain_head_mpv_ready(chapter, &pr.mpv)
        {
            if n.get() == 1 || n.get() % 15 == 0 {
                crate::preview_debug::log(format!(
                    "pump run={run_id} tick={}: waiting chain-head duration ({})",
                    n.get(),
                    crate::preview_debug::mpv_line(&pr.mpv)
                ));
            }
            return glib::ControlFlow::Continue;
        }
        let t = cap_preview_seek_time(seek_sec, content_dur);
        let seek_ok = preview_run_seek(&pr.mpv, &load_s, t, optical);
        crate::preview_debug::log(format!(
            "pump run={run_id} tick={} seek={t:.2} ok={seek_ok} gl={}x{} ({})",
            n.get(),
            gl2.width(),
            gl2.height(),
            crate::preview_debug::mpv_line(&pr.mpv)
        ));
        if !seek_ok {
            crate::preview_debug::warn(format!(
                "pump run={run_id} seek failed t={t:.2} optical={optical}"
            ));
        }
        if seek_ok {
            gl2.queue_render();
            crate::preview_debug::info(format!(
                "pump done run={run_id} tick={} seek={t:.2} ({})",
                n.get(),
                crate::preview_debug::mpv_line(&pr.mpv)
            ));
        }
        *pump2.borrow_mut() = None;
        glib::ControlFlow::Break
    });
    *pump.borrow_mut() = Some(id);
}

#[cfg(target_os = "macos")]
mod macos_compositing {
    include!("macos_compositing.rs");
}
