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
    /// User-visible preview; hide uses `set_visible(false)` only.
    pub shown: Rc<Cell<bool>>,
    pub bottom: gtk::Box,
    pub ovl: gtk::Overlay,
}

impl SeekPreviewState {
    pub(crate) fn clear_preview_visual(&self) {
        if let Some(pr) = self.preview.borrow().as_ref() {
            pr.clear_framebuffer(&self.gl);
        }
    }

    /// Auxiliary player still has the cached clip decoded and ready to render.
    pub(crate) fn preview_media_warm(&self) -> bool {
        if self.loaded_target.borrow().is_none() {
            return false;
        }
        let g = self.preview.borrow();
        let Some(pr) = g.as_ref() else {
            return false;
        };
        pr.mpv.get_property::<bool>("vo-configured") == Ok(true)
    }

    pub(crate) fn is_open(&self) -> bool {
        self.shown.get()
    }

    pub(crate) fn show_at(&self, x: f64) {
        let reopening = !self.shown.get();
        let (margin_start, margin_bottom) = self.show_margins(x);
        self.container.set_margin_start(margin_start);
        self.container.set_margin_bottom(margin_bottom);
        self.container.set_can_target(false);
        self.shown.set(true);
        self.container.set_visible(true);
        #[cfg(target_os = "macos")]
        if reopening {
            // Defensive: older theater hide used opacity 0; never show a transparent frame.
            self.container.set_opacity(1.0);
            macos_compositing::on_open(self);
        }
        if reopening && self.preview_media_warm() {
            self.gl.queue_render();
        }
    }

    /// Frame placement centered under the cursor, clamped to the seek overlay:
    /// returns (margin_start, margin_bottom).
    fn show_margins(&self, x: f64) -> (i32, i32) {
        // frame: padding 3px + border 1px per side = 8px over gl width; use allocated width when ready.
        let preview_w = self.preview_frame_w();
        let ovl_w = self.ovl.width().max(1) as f64;
        let raw = (self.cursor_x_in_overlay(x) - preview_w / 2.0).round();
        let margin_start = raw.clamp(0.0, (ovl_w - preview_w).max(0.0)) as i32;
        let margin_bottom = self.bottom.height().max(1) + PREVIEW_GAP;
        (margin_start, margin_bottom)
    }

    /// Allocated preview width (frame padding/border included), at least 1 px.
    /// Padding 3px + border 1px per side = 8px over gl width.
    fn preview_frame_w(&self) -> f64 {
        (self
            .container
            .width()
            .max(self.gl.width_request() + 8)
            .max(1)) as f64
    }

    fn cursor_x_in_overlay(&self, x: f64) -> f64 {
        self.seek
            .compute_point(&self.ovl, &gtk::graphene::Point::new(x as f32, 0.0))
            .map(|p| p.x() as f64)
            .unwrap_or(x)
    }

    pub(crate) fn hide(&self) {
        if !self.shown.replace(false) {
            return;
        }
        self.container.set_visible(false);
        self.container.set_can_target(false);
        #[cfg(target_os = "macos")]
        macos_compositing::on_close();
    }

    /// Main player opened another file — drop cached load target and hide until re-hover.
    pub(crate) fn reset_for_new_media(&self, from: &'static str) {
        self.log_reset_from(from);
        self.serial.set(self.serial.get().wrapping_add(1));
        self.drop_cached_media();
        self.hide();
    }

    /// Drops debounced/pump sources, cached load bookkeeping, and aux-player decode state.
    fn drop_cached_media(&self) {
        crate::glib_source_drop::drop_glib_source(self.deb.as_ref());
        crate::glib_source_drop::drop_glib_source(self.pump.as_ref());
        *self.loaded_target.borrow_mut() = None;
        *self.loaded_path.borrow_mut() = None;
        *self.preview_owner_db.borrow_mut() = None;
        *self.last_xy.borrow_mut() = None;
        if let Some(pr) = self.preview.borrow().as_ref() {
            reset_preview_player_decode(&pr.mpv);
            self.clear_preview_visual();
        }
    }

    fn log_reset_from(&self, from: &'static str) {
        crate::preview_debug::info(format!(
            "reset from {from} (prev_target={:?} owner={:?} visible={})",
            self.loaded_target.borrow().as_deref(),
            self.preview_owner_db
                .borrow()
                .as_ref()
                .map(|p| p.display().to_string()),
            self.is_open()
        ));
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
    let (dw, dh) = main_player_video_dims(&st.player);
    let (req_w, req_h) = preview_size(dw, dh, preview_px(st.seek.width()));
    if st.gl.width_request() != req_w || st.gl.height_request() != req_h {
        st.gl.set_size_request(req_w, req_h);
    }
}

/// Display dimensions of the main player's current video (fallback 1080p when unknown).
fn main_player_video_dims(player: &Rc<RefCell<Option<MpvBundle>>>) -> (i32, i32) {
    player
        .borrow()
        .as_ref()
        .map(|b| {
            let dw = b.mpv.get_property::<i64>("dwidth").unwrap_or(0) as i32;
            let dh = b.mpv.get_property::<i64>("dheight").unwrap_or(0) as i32;
            (dw.max(1), dh.max(1))
        })
        .unwrap_or((1920, 1080))
}

include!("preview_frame_pump.rs");

#[cfg(target_os = "macos")]
mod macos_compositing {
    include!("macos_compositing.rs");
}
