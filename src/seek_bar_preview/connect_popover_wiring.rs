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
    chapters: Rc<RefCell<Vec<(f64, String)>>>,
    dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
    ctx: SeekPreviewCtx,
) -> Rc<SeekPreviewState> {
    let SeekPreviewCtx { ovl, bottom } = ctx;
    let gl = gtk::GLArea::new();
    gl.set_auto_render(false);
    gl.set_has_stencil_buffer(false);
    gl.set_has_depth_buffer(false);
    gl.set_can_focus(false);
    gl.set_focus_on_click(false);
    gl.set_size_request(180, 101);

    let chapter_lbl = gtk::Label::new(None::<&str>);
    chapter_lbl.add_css_class("rp-seek-thumb-chapter");
    chapter_lbl.set_xalign(0.5);
    chapter_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    chapter_lbl.set_max_width_chars(28);
    chapter_lbl.set_visible(false);

    let time_lbl = gtk::Label::new(None::<&str>);
    time_lbl.add_css_class("rp-seek-thumb-time");
    time_lbl.add_css_class("numeric");
    time_lbl.set_xalign(0.5);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    body.append(&gl);
    body.append(&chapter_lbl);
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
        if pr_realize.borrow().is_some() {
            return;
        }
        match MpvPreviewGl::new(a) {
            Ok(p) => *pr_realize.borrow_mut() = Some(p),
            Err(e) => eprintln!("[rhino] seek preview GL/mpv: {e}"),
        }
    });
    let pr_unrealize = Rc::clone(&preview);
    gl.connect_unrealize(move |a| {
        a.make_current();
        if let Some(old) = pr_unrealize.borrow_mut().take() {
            old.dispose(a);
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
        chapter_lbl,
        time_lbl,
        preview,
        pump: Rc::new(RefCell::new(None)),
        serial: Rc::new(Cell::new(0)),
        loaded_path: Rc::new(RefCell::new(None)),
        loaded_target: Rc::new(RefCell::new(None)),
        enabled,
        seek: seek.clone(),
        seek_adj: seek_adj.clone(),
        player,
        last_path,
        chapters,
        dvd_bar,
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
        move |_, x, y| {
            if st.last_xy.borrow().is_some_and(|p| p == (x, y)) {
                return;
            }
            *st.last_xy.borrow_mut() = Some((x, y));

            let bar_d = st.seek_adj.upper();
            if bar_d <= 0.0 {
                st.hide();
                return;
            }
            let t = {
                let main = st.player.borrow();
                let preview = st.preview.borrow();
                seek_bar_label_time(
                    bar_d,
                    st.seek.width(),
                    x,
                    main.as_ref().map(|b| &b.mpv),
                    preview.as_ref().map(|p| &p.mpv),
                )
            };
            let Some(t) = t else {
                st.hide();
                return;
            };
            st.hover_t.set(t);
            st.time_lbl.set_text(&format_time(t));
            {
                let ch = st.chapters.borrow();
                let name = ch
                    .iter()
                    .rfind(|(ct, _)| *ct <= t)
                    .map(|(_, n)| n.as_str())
                    .unwrap_or("");
                st.chapter_lbl.set_text(name);
                st.chapter_lbl.set_visible(!name.is_empty());
            }

            if !st.enabled.get() {
                return;
            }

            set_preview_size(&st);

            if preview_open_path(&st.player, &st.last_path).is_none() {
                st.hide();
                return;
            }

            st.show_at(x);
            crate::glib_source_drop::drop_glib_source(st.deb.as_ref());
            crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
            schedule_preview_seek(Rc::clone(&st));
        }
    ));

    mot.connect_leave(glib::clone!(
        #[strong]
        st,
        move |_| {
            st.serial.set(st.serial.get().wrapping_add(1));
            crate::glib_source_drop::drop_glib_source(st.deb.as_ref());
            crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
            *st.last_xy.borrow_mut() = None;
            *st.loaded_target.borrow_mut() = None;
            *st.loaded_path.borrow_mut() = None;
            st.hide();
        }
    ));

    seek.add_controller(mot);
    st
}
