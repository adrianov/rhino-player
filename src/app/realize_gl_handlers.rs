fn mpv_gl_realize_attach(
    area: &gtk::GLArea,
    ok_refs: &GlRealizeOkRefs,
    file_boot_rz: &Rc<RefCell<Option<PathBuf>>>,
    vp_realize: &Rc<RefCell<db::VideoPrefs>>,
) {
    area.make_current();
    let skip_preload_followups = file_boot_rz.borrow().is_some();
    let init = {
        let mut v = vp_realize.borrow_mut();
        MpvBundle::new(area, &mut v)
    };
    match init {
        Ok((b, auto_off)) => gl_realize_bundle_ready(
            area,
            ok_refs,
            file_boot_rz,
            skip_preload_followups,
            b,
            auto_off,
        ),
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
