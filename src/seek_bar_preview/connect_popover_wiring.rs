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
            let main_dur = st
                .player
                .borrow()
                .as_ref()
                .map(|b| {
                    preview_hover_duration(
                        bar_d,
                        &b.mpv,
                        st.preview.borrow().as_ref().map(|p| &p.mpv),
                    )
                })
                .unwrap_or(bar_d);
            if main_dur <= 0.0 {
                st.hide();
                return;
            }

            let t = cap_preview_seek_time(
                (x / f64::from(st.seek.width().max(1))).clamp(0.0, 1.0) * bar_d,
                main_dur,
            );
            st.hover_t.set(t);
            st.time_lbl.set_text(&format_time(t));
            let ch = st.chapters.borrow();
            let name = ch.iter().rfind(|(ct, _)| *ct <= t).map(|(_, n)| n.as_str()).unwrap_or("");
            st.chapter_lbl.set_text(name);
            st.chapter_lbl.set_visible(!name.is_empty());
            drop(ch);

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

fn preview_open_path(
    player: &Rc<RefCell<Option<MpvBundle>>>,
    last_path: &Rc<RefCell<Option<PathBuf>>>,
) -> Option<PathBuf> {
    let g = player.borrow();
    let b = g.as_ref()?;
    let shell = b.me_budget_shell_path.borrow();
    if let Some(s) = preview_load_path(&b.mpv, shell.as_deref()) {
        return Some(preview_cache_path(&s));
    }
    let raw = last_path.borrow().clone()?;
    if !crate::video_ext::is_openable_media_path(&raw) {
        return None;
    }
    let resolved = crate::video_ext::resolve_open_media_path(&raw);
    resolved.to_str().map(preview_cache_path)
}

fn schedule_preview_seek(st: Rc<SeekPreviewState>) {
    let run_id = st.serial.get().wrapping_add(1);
    st.serial.set(run_id);
    let st2 = Rc::clone(&st);
    let id = glib::source::timeout_add_local_full(
        PREVIEW_DEBOUNCE,
        glib::Priority::LOW,
        move || {
            let _ = st2.deb.borrow_mut().take();
            if st2.serial.get() != run_id || !st2.enabled.get() {
                return glib::ControlFlow::Break;
            }
            let load_s = {
                let g = st2.player.borrow();
                let Some(b) = g.as_ref() else {
                    st2.hide();
                    return glib::ControlFlow::Break;
                };
                let shell = b.me_budget_shell_path.borrow().clone();
                preview_load_path(&b.mpv, shell.as_deref())
            };
            let Some(load_s) = load_s else {
                st2.hide();
                return glib::ControlFlow::Break;
            };
            let bar_d = st2.seek_adj.upper();
            let content_dur = st2
                .player
                .borrow()
                .as_ref()
                .map(|b| {
                    preview_hover_duration(
                        bar_d,
                        &b.mpv,
                        st2.preview.borrow().as_ref().map(|p| &p.mpv),
                    )
                })
                .unwrap_or(bar_d);
            let t = cap_preview_seek_time(st2.hover_t.get(), content_dur);
            do_preview_seek(&st2, &load_s, content_dur, t, run_id);
            glib::ControlFlow::Break
        },
    );
    *st.deb.borrow_mut() = Some(id);
}

fn do_preview_seek(
    st: &Rc<SeekPreviewState>,
    load_s: &str,
    content_dur: f64,
    t: f64,
    run_id: u64,
) {
    let mut g = st.preview.borrow_mut();
    let Some(pr) = g.as_mut() else {
        return;
    };
    if load_s.is_empty() {
        return;
    }
    let cache = preview_cache_path(load_s);
    let need_load = st.loaded_target.borrow().as_deref() != Some(load_s);
    let optical = preview_media_is_optical(load_s);

    if need_load {
        prepare_preview_player(&pr.mpv, load_s);
        if let Err(e) = pr.mpv.command("loadfile", &[load_s, "replace"]) {
            eprintln!("[rhino] seek preview: loadfile failed: {e:?} ({load_s})");
            return;
        }
        *st.loaded_path.borrow_mut() = Some(cache);
        *st.loaded_target.borrow_mut() = Some(load_s.to_string());
        drop(g);
        start_preview_frame_pump(
            &st.gl,
            &st.preview,
            &st.pump,
            &st.serial,
            run_id,
            content_dur,
            t,
            optical,
        );
    } else {
        set_preview_tracks(&pr.mpv);
        drop(g);
        start_preview_frame_pump(
            &st.gl,
            &st.preview,
            &st.pump,
            &st.serial,
            run_id,
            content_dur,
            t,
            optical,
        );
    }
}
