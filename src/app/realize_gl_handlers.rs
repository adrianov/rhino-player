fn mpv_gl_realize_attach(
    area: &gtk::GLArea,
    ok_refs: &GlRealizeOkRefs,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    vp_realize: &Rc<RefCell<db::VideoPrefs>>,
) {
    let area = area.clone();
    let ok_refs = GlRealizeOkRefs {
        p_realize: ok_refs.p_realize.clone(),
        vp_realize: Rc::clone(&ok_refs.vp_realize),
        app_realize: ok_refs.app_realize.clone(),
        sp_realize: ok_refs.sp_realize.clone(),
        recent_rz: ok_refs.recent_rz.clone(),
        last_rz: ok_refs.last_rz.clone(),
        reapply_rz: ok_refs.reapply_rz.clone(),
        pending_rz: ok_refs.pending_rz.clone(),
        close_video: ok_refs.close_video.clone(),
        move_to_trash: ok_refs.move_to_trash.clone(),
        bottom_rz: ok_refs.bottom_rz.clone(),
        bshow_rz: ok_refs.bshow_rz.clone(),
        win_rz: ok_refs.win_rz.clone(),
        gl_rz: ok_refs.gl_rz.clone(),
        on_vid_rz: ok_refs.on_vid_rz.clone(),
        wa_st: ok_refs.wa_st.clone(),
        ol_rz: ok_refs.ol_rz.clone(),
        hdr_title_mirror: ok_refs.hdr_title_mirror.clone(),
        playback_focus: Rc::clone(&ok_refs.playback_focus),
        close_video_btn: ok_refs.close_video_btn.clone(),
    };
    let file_boot_rz = Rc::clone(file_boot_rz);
    let vp_realize = Rc::clone(vp_realize);
    glib::idle_add_local_once(move || mpv_gl_realize_attach_now(
        &area,
        &ok_refs,
        &file_boot_rz,
        &vp_realize,
    ));
}

fn mpv_gl_realize_attach_now(
    area: &gtk::GLArea,
    ok_refs: &GlRealizeOkRefs,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    vp_realize: &Rc<RefCell<db::VideoPrefs>>,
) {
    area.make_current();
    let init = {
        let mut v = vp_realize.borrow_mut();
        MpvBundle::new(area, &mut v)
    };
    match init {
        Ok((b, auto_off)) => {
            gl_realize_bundle_ready(area, ok_refs, b, auto_off);
            run_stashed_after_present_wire();
            if let Some(p) = file_boot_rz.replace(None) {
                let mut o = LoadOpts::replace_media(ReplaceMediaBundled {
                    video_pref: Rc::clone(&vp_realize),
                    last_path: ok_refs.last_rz.clone(),
                    on_start: Some(Rc::clone(&ok_refs.on_vid_rz)),
                    win_aspect: Rc::clone(&ok_refs.wa_st),
                    on_loaded: Some(Rc::clone(&ok_refs.ol_rz)),
                    play_on_start: false,
                    reset_speed_to_normal: false,
                    hdr_title_mirror: ok_refs.hdr_title_mirror.clone(),
                });
                o.playback_focus = Some(Rc::clone(&ok_refs.playback_focus));
                if let Err(e) = try_load(
                    &p,
                    &ok_refs.p_realize,
                    &ok_refs.win_rz,
                    &ok_refs.gl_rz,
                    &ok_refs.recent_rz,
                    &o,
                ) {
                    eprintln!("[rhino] try_load (startup): {e}");
                }
            }
        }
        Err(e) => eprintln!("[rhino] OpenGL / mpv: {e}"),
    }
}

/// Linux passes `win_hide = Some(window)` so teardown can hide the GTK shell before quit; macOS passes `None`.
fn mpv_gl_render_frame(
    area: &gtk::GLArea,
    td: &Rc<Cell<bool>>,
    p_draw: &Rc<RefCell<Option<MpvBundle>>>,
    app_rd: &adw::Application,
    gl_bundle_drop: &gtk::GLArea,
    win_hide: Option<&adw::ApplicationWindow>,
) -> glib::Propagation {
    area.make_current();
    if td.replace(false) {
        if let Some(b) = p_draw.borrow().as_ref() {
            b.teardown_gl_paint(area);
        }
        if let Some(w) = win_hide {
            w.set_visible(false);
        }
        let to_drop = p_draw.borrow_mut().take();
        let app_q = app_rd.clone();
        let gl_q = gl_bundle_drop.clone();
        glib::idle_add_local_once(move || {
            gl_q.make_current();
            if let Some(b) = to_drop {
                b.dispose_for_quit(&gl_q);
            }
            app_q.quit();
        });
        return glib::Propagation::Stop;
    }
    #[cfg(target_os = "macos")]
    if let Some(b) = p_draw.borrow().as_ref() {
        b.clear_glarea_transparent();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(b) = p_draw.borrow().as_ref() {
        b.draw(area);
    }
    glib::Propagation::Stop
}
