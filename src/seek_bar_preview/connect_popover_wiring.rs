pub struct SeekPreviewCtx {
    pub ovl: gtk::Overlay,
    pub bottom: gtk::Box,
}

pub fn connect(
    seek: &gtk::Scale,
    seek_adj: &gtk::Adjustment,
    player: Rc<RefCell<Option<MpvBundle>>>,
    last_path: Rc<RefCell<Option<PathBuf>>>,
    enabled: Rc<Cell<bool>>,
    seek_grabbed: Rc<Cell<bool>>,
    ctx: SeekPreviewCtx,
) -> Rc<SeekPreviewState> {
    let SeekPreviewCtx { ovl, bottom } = ctx;
    let gl = gtk::GLArea::new();
    gl.set_auto_render(false);
    gl.set_has_stencil_buffer(false);
    gl.set_has_depth_buffer(false);
    gl.set_size_request(180, 101);

    let time_lbl = gtk::Label::new(None::<&str>);
    time_lbl.add_css_class("rp-seek-thumb-time");
    time_lbl.add_css_class("numeric");
    time_lbl.set_xalign(0.5);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.append(&gl);
    body.append(&time_lbl);

    let container = gtk::Frame::new(None::<&str>);
    container.add_css_class("rp-seek-thumb-frame");
    container.set_child(Some(&body));
    container.set_halign(gtk::Align::Start);
    container.set_valign(gtk::Align::End);
    container.set_visible(false);
    container.set_can_target(false);

    let preview = Rc::new(RefCell::new(None::<MpvPreviewGl>));
    let pr_realize = Rc::clone(&preview);
    gl.connect_realize(move |a| {
        a.make_current();
        match MpvPreviewGl::new(a) {
            Ok(p) => *pr_realize.borrow_mut() = Some(p),
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
        container,
        gl,
        time_lbl,
        preview,
        pump: Rc::new(RefCell::new(None)),
        serial: Rc::new(Cell::new(0)),
        loaded_path: Rc::new(RefCell::new(None)),
        enabled,
        seek: seek.clone(),
        seek_adj: seek_adj.clone(),
        player,
        last_path,
        hover_t: Rc::new(Cell::new(0.0)),
        last_xy: Rc::new(RefCell::new(None)),
        deb: Rc::new(RefCell::new(None)),
        bottom,
        ovl,
    });

    let mot = gtk::EventControllerMotion::new();

    mot.connect_motion(glib::clone!(
        #[strong]
        st,
        #[strong]
        seek_grabbed,
        move |_, x, y| {
            if st.last_xy.borrow().is_some_and(|p| p == (x, y)) {
                return;
            }
            *st.last_xy.borrow_mut() = Some((x, y));

            let dur = st.seek_adj.upper();
            if dur <= 0.0 || !st.enabled.get() {
                st.hide();
                return;
            }

            let t = if seek_grabbed.get() {
                st.seek.value().clamp(0.0, dur)
            } else {
                (x / f64::from(st.seek.width().max(1))).clamp(0.0, 1.0) * dur
            };
            st.hover_t.set(t);
            st.time_lbl.set_text(&format_time(t));
            set_preview_size(&st);

            let path_ok = st
                .player
                .borrow()
                .as_ref()
                .and_then(|b| local_file_from_mpv(&b.mpv))
                .or_else(|| st.last_path.borrow().clone())
                .is_some_and(|p| p.is_file());
            if !path_ok {
                st.hide();
                return;
            }

            st.show_at(x);
            st.serial.set(st.serial.get().wrapping_add(1));
            if let Some(id) = st.deb.borrow_mut().take() {
                id.remove();
            }
            if let Some(id) = st.pump.borrow_mut().take() {
                id.remove();
            }
            schedule_preview_seek(Rc::clone(&st));
        }
    ));

    mot.connect_leave(glib::clone!(
        #[strong]
        st,
        move |_| {
            st.serial.set(st.serial.get().wrapping_add(1));
            if let Some(id) = st.deb.borrow_mut().take() {
                id.remove();
            }
            if let Some(id) = st.pump.borrow_mut().take() {
                id.remove();
            }
            *st.last_xy.borrow_mut() = None;
            st.hide();
        }
    ));

    seek.add_controller(mot);
    st
}

fn schedule_preview_seek(st: Rc<SeekPreviewState>) {
    let run_id = st.serial.get();
    let st2 = Rc::clone(&st);
    let id = glib::source::timeout_add_local_full(
        PREVIEW_DEBOUNCE,
        glib::Priority::LOW,
        move || {
            let _ = st2.deb.borrow_mut().take();
            if st2.serial.get() != run_id || !st2.enabled.get() {
                return glib::ControlFlow::Break;
            }
            let pth = st2
                .player
                .borrow()
                .as_ref()
                .and_then(|b| local_file_from_mpv(&b.mpv))
                .or_else(|| st2.last_path.borrow().clone())
                .filter(|p| p.is_file());
            let Some(pth) = pth else {
                st2.hide();
                return glib::ControlFlow::Break;
            };
            let canon = std::fs::canonicalize(&pth).unwrap_or(pth);
            let up = st2.seek_adj.upper();
            let mpv_d = st2
                .player
                .borrow()
                .as_ref()
                .and_then(|b| b.mpv.get_property::<f64>("duration").ok())
                .filter(|d| d.is_finite() && *d > 0.0)
                .unwrap_or(up);
            let t = st2.hover_t.get().clamp(0.0, (mpv_d - 0.01).max(0.0));
            do_preview_seek(&st2, &canon, t, run_id);
            glib::ControlFlow::Break
        },
    );
    *st.deb.borrow_mut() = Some(id);
}

fn do_preview_seek(st: &Rc<SeekPreviewState>, canon: &std::path::Path, t: f64, run_id: u64) {
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else {
        return;
    };
    let need_load = st
        .loaded_path
        .borrow()
        .as_ref()
        .map(|c| c != canon)
        .unwrap_or(true);

    if need_load {
        let Some(s) = canon.to_str() else {
            return;
        };
        if pr.mpv.command("loadfile", &[s, "replace"]).is_err() {
            return;
        }
        set_preview_tracks(&pr.mpv);
        *st.loaded_path.borrow_mut() = Some(canon.to_path_buf());
        drop(g);
        start_vo_pump(&st.gl, &st.preview, &st.pump, &st.serial, run_id, t);
    } else {
        set_preview_tracks(&pr.mpv);
        let t_s = format!("{t:.3}");
        let _ = pr.mpv.command("seek", &[t_s.as_str(), "absolute+keyframes"]);
        drop(g);
        st.gl.queue_render();
    }
}
