pub struct SeekPreviewCtx {
    pub ovl: gtk::Overlay,
    /// Bottom chrome used for preview lift (`bottom_shell` on macOS, row elsewhere).
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
    let loaded_path = Rc::new(RefCell::new(None::<PathBuf>));
    let loaded_target = Rc::new(RefCell::new(None::<String>));
    let preview_owner_db = Rc::new(RefCell::new(None::<PathBuf>));

    let st = Rc::new(SeekPreviewState {
        container,
        gl,
        chapter_lbl,
        time_lbl,
        preview,
        pump: Rc::new(RefCell::new(None)),
        serial: Rc::new(Cell::new(0)),
        loaded_path,
        loaded_target,
        preview_owner_db,
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
        shown: Rc::new(Cell::new(false)),
        bottom,
        ovl,
    });

    wire_preview_gl(&st);
    #[cfg(target_os = "macos")]
    macos_compositing::wire_opaque_frame(&st);

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
                crate::preview_debug::warn(format!("motion: bar upper={bar_d} — hide"));
                st.hide();
                return;
            }
            let t = {
                let main = st.player.borrow();
                let shell = main
                    .as_ref()
                    .and_then(|b| b.me_budget_shell_path.borrow().clone());
                let preview = st.preview.borrow();
                seek_bar_label_time(
                    bar_d,
                    st.seek.width(),
                    x,
                    main.as_ref().map(|b| &b.mpv),
                    shell.as_deref(),
                    preview.as_ref().map(|p| &p.mpv),
                    Some(&st.dvd_bar),
                )
            };
            let Some(t) = t else {
                crate::preview_debug::warn(format!(
                    "motion: no hover time bar={bar_d:.2} w={}",
                    st.seek.width()
                ));
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
                crate::preview_debug::info("motion: preview off in prefs — labels only");
                return;
            }

            set_preview_size(&st);

            if preview_open_path(&st.player, &st.last_path).is_none() {
                crate::preview_debug::warn("motion: open target not ready — hide");
                st.hide();
                return;
            }

            let reopening = !st.is_open();
            st.show_at(x);
            crate::glib_source_drop::drop_glib_source(st.pump.as_ref());
            if reopening {
                crate::preview_debug::info(format!(
                    "reopen warm={} hover={:.2}",
                    st.preview_media_warm(),
                    st.hover_t.get()
                ));
                run_preview_seek_now(&st);
            } else {
                arm_preview_debounce(Rc::clone(&st));
            }
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
            st.hide();
        }
    ));

    seek.add_controller(mot);
    register(Rc::clone(&st));
    st
}
